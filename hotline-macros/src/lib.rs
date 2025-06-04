use proc_macro::TokenStream;
use proc_macro_error2::proc_macro_error;
use quote::{ToTokens, quote};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::process::Command;
use syn::{
    Fields, FnArg, GenericArgument, ImplItem, ItemImpl, ItemStruct, Pat, PathArguments, ReturnType, Type, braced,
    parse::{Parse, ParseStream},
};

// ===== Core Types =====

struct ObjectInput {
    struct_item: ItemStruct,
    impl_blocks: Vec<ItemImpl>,
}

impl Parse for ObjectInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        braced!(content in input);

        let struct_item: ItemStruct =
            content.parse().map_err(|e| syn::Error::new(e.span(), "Expected a struct definition"))?;
        
        let mut impl_blocks = Vec::new();
        while !content.is_empty() {
            impl_blocks.push(content.parse::<ItemImpl>()?);
        }

        if impl_blocks.is_empty() {
            return Err(syn::Error::new(content.span(), "Expected at least one impl block after the struct"));
        }

        Ok(ObjectInput { struct_item, impl_blocks })
    }
}

#[derive(Debug, Clone, PartialEq)]
enum ReceiverType {
    Value,
    Ref,
    RefMut,
}

// ===== Symbol Name Builder =====

#[derive(Clone)]
struct SymbolName {
    type_name: String,
    method_name: String,
    params: Vec<(String, String)>,
    return_type: String,
    rustc_commit: String,
}

impl SymbolName {
    fn new(type_name: &str, method_name: &str, rustc_commit: &str) -> Self {
        Self {
            type_name: type_name.to_string(),
            method_name: method_name.to_string(),
            params: Vec::new(),
            return_type: "unit".to_string(),
            rustc_commit: rustc_commit.to_string(),
        }
    }

    fn with_params(mut self, params: Vec<(String, String)>) -> Self {
        self.params = params;
        self
    }

    fn with_return_type(mut self, return_type: String) -> Self {
        self.return_type = return_type;
        self
    }

    fn build_method(&self) -> String {
        let mut parts = vec![
            self.type_name.clone(),
            self.method_name.clone(),
            "____obj_mut_dyn_Any".to_string(),
        ];
        
        for (name, ty) in &self.params {
            parts.push(format!("__{}__{}", name, ty));
        }
        
        parts.push(format!("__to__{}", self.return_type));
        parts.push(self.rustc_commit.clone());
        
        parts.join("__")
    }

    fn build_getter(&self) -> String {
        format!("{}__{}______obj_mut_dyn_Any____to__{}__{}",
            self.type_name, self.method_name, self.return_type, self.rustc_commit)
    }

    fn build_setter(&self, field_name: &str, field_type: &str) -> String {
        format!("{}__set_{}____obj_mut_dyn_Any__{}_{}__to__unit__{}",
            self.type_name, field_name, field_name, field_type, self.rustc_commit)
    }

    fn build_constructor(&self) -> String {
        format!("{}__new____to__Box_lt_dyn_HotlineObject_gt__{}", self.type_name, self.rustc_commit)
    }

    fn build_type_name_getter(&self) -> String {
        format!("{}__get_type_name____obj_ref_dyn_Any__to__str__{}", self.type_name, self.rustc_commit)
    }

    fn build_init(&self) -> String {
        format!("{}__init__registry__{}", self.type_name, self.rustc_commit)
    }
}

// ===== Type Processing =====

fn type_to_string(ty: &Type) -> String {
    // Handle Like<T> specially
    if let Some(inner_ty) = extract_like_type(ty) {
        return type_to_string(inner_ty);
    }
    
    match ty {
        Type::Path(type_path) => path_to_string(type_path),
        Type::Reference(type_ref) => {
            format!("{}ref_{}", 
                if type_ref.mutability.is_some() { "mut_" } else { "" },
                type_to_string(&type_ref.elem))
        }
        Type::Slice(type_slice) => format!("slice_{}", type_to_string(&type_slice.elem)),
        Type::Array(type_array) => format!("array_{}", type_to_string(&type_array.elem)),
        Type::Tuple(type_tuple) if type_tuple.elems.is_empty() => "unit".to_string(),
        Type::Tuple(type_tuple) => {
            let elems: Vec<_> = type_tuple.elems.iter()
                .map(type_to_string)
                .collect();
            format!("tuple_{}", elems.join("_comma_"))
        }
        _ => panic!("Unsupported type in hotline macro: {:?}", ty),
    }
}

fn path_to_string(type_path: &syn::TypePath) -> String {
    if let Some(ident) = type_path.path.get_ident() {
        return ident.to_string();
    }
    
    let mut result = String::new();
    for (i, segment) in type_path.path.segments.iter().enumerate() {
        if i > 0 {
            result.push_str("_");
        }
        result.push_str(&segment.ident.to_string());

        if let PathArguments::AngleBracketed(args) = &segment.arguments {
            result.push_str("_lt_");
            let types: Vec<_> = args.args.iter()
                .filter_map(|arg| match arg {
                    GenericArgument::Type(ty) => Some(type_to_string(ty)),
                    _ => None,
                })
                .collect();
            result.push_str(&types.join("_"));
            result.push_str("_gt");
        }
    }
    result
}

fn is_standard_type(name: &str) -> bool {
    matches!(
        name,
        "String" | "Vec" | "Option" | "Result" | "Box" | "Arc" | "Mutex" | 
        "HashMap" | "BTreeMap" | "HashSet" | "BTreeSet" | "Cell" | "RefCell" | 
        "Rc" | "Weak" | "PhantomData" | "Pin" | "Future" | "Stream"
    )
}

fn is_object_type(name: &str) -> bool {
    name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) && !is_standard_type(name)
}

fn needs_typed_wrapper(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(ident) = type_path.path.get_ident() {
            return is_object_type(&ident.to_string());
        }
    }
    false
}

fn is_generic_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        type_path.path.segments.iter().any(|seg| 
            matches!(seg.arguments, PathArguments::AngleBracketed(_)))
    } else {
        false
    }
}

fn extract_generic_inner_type<'a>(ty: &'a Type, type_name: &str) -> Option<&'a Type> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == type_name {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                        return Some(inner_ty);
                    }
                }
            }
        }
    }
    None
}

fn extract_option_type(ty: &Type) -> Option<&Type> {
    extract_generic_inner_type(ty, "Option")
}

fn extract_like_type(ty: &Type) -> Option<&Type> {
    extract_generic_inner_type(ty, "Like")
}

fn resolve_self_type(ty: Type, type_name: &str) -> Type {
    use syn::visit_mut::{self, VisitMut};
    
    struct SelfResolver<'a> {
        type_name: &'a str,
    }
    
    impl<'a> VisitMut for SelfResolver<'a> {
        fn visit_type_mut(&mut self, ty: &mut Type) {
            if let Type::Path(type_path) = ty {
                if type_path.path.is_ident("Self") {
                    *ty = syn::parse_str(self.type_name).expect("Failed to parse type name");
                    return;
                }
            }
            visit_mut::visit_type_mut(self, ty);
        }
    }
    
    let mut ty = ty;
    SelfResolver { type_name }.visit_type_mut(&mut ty);
    ty
}

// ===== Type Discovery =====

fn find_referenced_object_types(struct_item: &ItemStruct, impl_blocks: &[ItemImpl]) -> HashSet<String> {
    use syn::visit::{self, Visit};

    struct TypeVisitor {
        types: HashSet<String>,
        current_type: String,
    }

    impl<'ast> Visit<'ast> for TypeVisitor {
        fn visit_type(&mut self, ty: &'ast Type) {
            if let Some(inner_ty) = extract_like_type(ty) {
                self.visit_type(inner_ty);
                return;
            }
            
            if let Type::Path(type_path) = ty {
                if let Some(ident) = type_path.path.get_ident() {
                    let name = ident.to_string();
                    if is_object_type(&name) && name != self.current_type && name != "Self" {
                        self.types.insert(name);
                    }
                }
            }
            visit::visit_type(self, ty);
        }

        fn visit_expr(&mut self, expr: &'ast syn::Expr) {
            // String literals that might be object names
            if let syn::Expr::Lit(expr_lit) = expr {
                if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                    let value = lit_str.value();
                    if is_object_type(&value) && value != self.current_type {
                        self.types.insert(value);
                    }
                }
            }
            
            // ObjectType::new() calls
            if let syn::Expr::Call(expr_call) = expr {
                if let syn::Expr::Path(path_expr) = &*expr_call.func {
                    if path_expr.path.segments.len() == 2 {
                        let type_name = path_expr.path.segments[0].ident.to_string();
                        let method_name = path_expr.path.segments[1].ident.to_string();
                        if method_name == "new" && is_object_type(&type_name) && type_name != self.current_type {
                            self.types.insert(type_name);
                        }
                    }
                }
            }
            
            visit::visit_expr(self, expr);
        }
    }

    let mut visitor = TypeVisitor { 
        types: HashSet::new(), 
        current_type: struct_item.ident.to_string() 
    };

    visitor.visit_item_struct(struct_item);
    for impl_block in impl_blocks {
        visitor.visit_item_impl(impl_block);
    }

    visitor.types
}

fn find_object_lib_file(type_name: &str) -> std::path::PathBuf {
    let mut current_dir = std::env::current_dir().unwrap();
    loop {
        if current_dir.join("Cargo.toml").exists() && current_dir.join("objects").exists() {
            break;
        }
        if !current_dir.pop() {
            current_dir = std::env::current_dir().unwrap();
            if let Ok(out_dir) = std::env::var("OUT_DIR") {
                let out_path = Path::new(&out_dir);
                if let Some(target_dir) = out_path.ancestors().find(|p| p.ends_with("target")) {
                    if let Some(workspace) = target_dir.parent() {
                        current_dir = workspace.to_path_buf();
                    }
                }
            }
            break;
        }
    }

    current_dir.join("objects").join(type_name).join("src").join("lib.rs")
}

// ===== Code Generation Helpers =====

fn generate_ffi_wrapper(
    struct_name: &syn::Ident,
    wrapper_name: syn::Ident,
    params: Vec<(syn::Ident, &Type)>,
    return_type: Option<&Type>,
    body: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let param_list = params.iter().map(|(name, ty)| quote! { #name: #ty });
    let return_spec = return_type.map(|ty| quote! { -> #ty }).unwrap_or_default();
    let panic_msg = format!("Type mismatch in {}: expected {}, but got {{}}", 
        wrapper_name, struct_name);
    
    quote! {
        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub extern "Rust" fn #wrapper_name(
            obj: &mut dyn ::std::any::Any
            #(, #param_list)*
        ) #return_spec {
            let instance = obj.downcast_mut::<#struct_name>()
                .unwrap_or_else(|| {
                    panic!(#panic_msg, ::std::any::type_name_of_val(&*obj))
                });
            #body
        }
    }
}

// ===== Component Generators =====

struct ProcessedStruct {
    modified_struct: ItemStruct,
    fields_with_setters: HashSet<String>,
    field_defaults: HashMap<String, syn::Expr>,
}

fn process_struct_attributes(struct_item: &ItemStruct) -> ProcessedStruct {
    let mut modified_struct = struct_item.clone();
    let mut fields_with_setters = HashSet::new();
    let mut field_defaults = HashMap::new();
    
    if let Fields::Named(fields) = &mut modified_struct.fields {
        for field in &mut fields.named {
            let mut has_setter = false;
            let mut default_value = None;
            
            field.attrs.retain(|attr| {
                if attr.path().is_ident("setter") {
                    has_setter = true;
                    false
                } else if attr.path().is_ident("default") {
                    if let Ok(value) = attr.parse_args::<syn::Expr>() {
                        default_value = Some(value);
                    }
                    false
                } else {
                    true
                }
            });
            
            if has_setter {
                if let Some(field_name) = &field.ident {
                    fields_with_setters.insert(field_name.to_string());
                }
            }
            
            if let (Some(field_name), Some(value)) = (&field.ident, default_value) {
                field_defaults.insert(field_name.to_string(), value);
            }
        }
    }
    
    ProcessedStruct {
        modified_struct,
        fields_with_setters,
        field_defaults,
    }
}

fn generate_field_accessors(
    struct_name: &syn::Ident,
    processed: &ProcessedStruct,
    rustc_commit: &str,
) -> Vec<proc_macro2::TokenStream> {
    let mut accessors = Vec::new();
    
    if let Fields::Named(fields) = &processed.modified_struct.fields {
        for field in &fields.named {
            let field_name = field.ident.as_ref().unwrap();
            let field_type = &field.ty;
            let is_public = matches!(field.vis, syn::Visibility::Public(_));

            if !is_generic_type(field_type) {
                let symbol = SymbolName::new(&struct_name.to_string(), &field_name.to_string(), rustc_commit);
                let type_str = type_to_string(field_type);

                // Getter for public fields
                if is_public {
                    let getter_name = quote::format_ident!("{}", symbol.clone().with_return_type(type_str.clone()).build_getter());
                    let body = quote! { instance.#field_name.clone() };
                    accessors.push(generate_ffi_wrapper(
                        struct_name,
                        getter_name,
                        vec![],
                        Some(field_type),
                        body,
                    ));
                }

                // Setter for fields with #[setter]
                if processed.fields_with_setters.contains(&field_name.to_string()) {
                    let setter_name = quote::format_ident!("{}", symbol.build_setter(&field_name.to_string(), &type_str));
                    let body = quote! { instance.#field_name = value; };
                    accessors.push(generate_ffi_wrapper(
                        struct_name,
                        setter_name,
                        vec![(quote::format_ident!("value"), field_type)],
                        None,
                        body,
                    ));
                }
            }
        }
    }
    
    accessors
}

fn generate_method_wrappers(
    struct_name: &syn::Ident,
    main_impl: &ItemImpl,
    processed: &ProcessedStruct,
    rustc_commit: &str,
) -> Vec<proc_macro2::TokenStream> {
    let mut wrappers = Vec::new();
    
    // Generate setter method wrappers for Option<ObjectType> fields
    if let Fields::Named(fields) = &processed.modified_struct.fields {
        for field in &fields.named {
            let field_name = field.ident.as_ref().unwrap();
            if processed.fields_with_setters.contains(&field_name.to_string()) {
                let field_type = &field.ty;
                let setter_name = quote::format_ident!("set_{}", field_name);
                
                if let Some(inner_type) = extract_option_type(field_type) {
                    if let Type::Path(type_path) = inner_type {
                        if let Some(ident) = type_path.path.get_ident() {
                            let type_name = ident.to_string();
                            if is_object_type(&type_name) {
                                let type_str = type_to_string(inner_type);
                                let wrapper_fn_name = quote::format_ident!(
                                    "{}__{}______obj_mut_dyn_Any____value__ref_{}____to__unit__{}",
                                    struct_name,
                                    setter_name,
                                    type_str,
                                    rustc_commit
                                );
                                
                                let body = quote! { instance.#setter_name(value) };
                                wrappers.push(generate_ffi_wrapper(
                                    struct_name,
                                    wrapper_fn_name,
                                    vec![(quote::format_ident!("value"), &syn::parse_quote! { &#inner_type })],
                                    None,
                                    body,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Generate wrappers for regular methods
    for item in &main_impl.items {
        if let ImplItem::Fn(method) = item {
            if !matches!(method.vis, syn::Visibility::Public(_)) {
                continue;
            }

            let method_name = &method.sig.ident;
            
            // Check receiver type
            if let Some(FnArg::Receiver(receiver)) = method.sig.inputs.first() {
                if receiver.reference.is_none() {
                    continue; // Skip methods that take self by value
                }
            } else {
                continue; // Skip methods without self
            }

            // Build parameters
            let mut params = Vec::new();
            let mut param_specs = Vec::new();
            
            for arg in method.sig.inputs.iter().skip(1) {
                if let FnArg::Typed(typed) = arg {
                    if let Pat::Ident(pat_ident) = &*typed.pat {
                        let arg_name = pat_ident.ident.clone();
                        let arg_type = &*typed.ty;
                        params.push((arg_name.clone(), arg_type));
                        param_specs.push((arg_name.to_string(), type_to_string(arg_type)));
                    }
                }
            }

            // Handle return type
            let return_type = match &method.sig.output {
                ReturnType::Default => None,
                ReturnType::Type(_, ty) => Some(resolve_self_type((**ty).clone(), &struct_name.to_string())),
            };
            
            let return_type_str = return_type.as_ref()
                .map(type_to_string)
                .unwrap_or_else(|| "unit".to_string());

            let symbol = SymbolName::new(&struct_name.to_string(), &method_name.to_string(), rustc_commit)
                .with_params(param_specs)
                .with_return_type(return_type_str);
            
            let wrapper_name = quote::format_ident!("{}", symbol.build_method());
            let arg_names: Vec<_> = params.iter().map(|(name, _)| name).collect();
            let body = quote! { instance.#method_name(#(#arg_names),*) };
            
            wrappers.push(generate_ffi_wrapper(
                struct_name,
                wrapper_name,
                params,
                return_type.as_ref(),
                body,
            ));
        }
    }
    
    wrappers
}

fn generate_core_functions(struct_name: &syn::Ident, rustc_commit: &str, has_default: bool) -> proc_macro2::TokenStream {
    let symbol = SymbolName::new(&struct_name.to_string(), "", rustc_commit);
    
    // Constructor
    let constructor = if has_default {
        let ctor_name = quote::format_ident!("{}", symbol.build_constructor());
        quote! {
            #[unsafe(no_mangle)]
            #[allow(non_snake_case)]
            pub extern "Rust" fn #ctor_name() -> Box<dyn ::hotline::HotlineObject> {
                Box::new(<#struct_name as Default>::default()) as Box<dyn ::hotline::HotlineObject>
            }
        }
    } else {
        quote! {}
    };
    
    // Type name getter
    let type_name_fn_name = quote::format_ident!("{}", symbol.build_type_name_getter());
    let type_name_fn = quote! {
        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub extern "Rust" fn #type_name_fn_name(obj: &dyn ::std::any::Any) -> &'static str {
            match obj.downcast_ref::<#struct_name>() {
                Some(_) => stringify!(#struct_name),
                None => {
                    let type_name: &'static str = ::std::any::type_name_of_val(obj);
                    panic!(
                        "Type mismatch in type_name getter: expected {}, but got {}",
                        stringify!(#struct_name),
                        type_name
                    )
                }
            }
        }
    };
    
    // Init function
    let init_fn_name = quote::format_ident!("{}", symbol.build_init());
    let init_fn = quote! {
        static mut LIBRARY_REGISTRY: Option<*const ::hotline::LibraryRegistry> = None;

        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub extern "C" fn #init_fn_name(registry: *const ::hotline::LibraryRegistry) {
            unsafe {
                LIBRARY_REGISTRY = Some(registry);
            }
        }

        pub fn with_library_registry<F, R>(f: F) -> Option<R>
        where
            F: FnOnce(&::hotline::LibraryRegistry) -> R,
        {
            unsafe {
                LIBRARY_REGISTRY.and_then(|ptr| ptr.as_ref()).map(f)
            }
        }
    };
    
    quote! {
        #constructor
        #type_name_fn
        #init_fn
    }
}

fn generate_setter_builder_methods(
    struct_name: &syn::Ident,
    processed: &ProcessedStruct,
) -> proc_macro2::TokenStream {
    let mut setter_methods = Vec::new();
    let mut builder_methods = Vec::new();
    
    if let Fields::Named(fields) = &processed.modified_struct.fields {
        for field in &fields.named {
            let field_name = field.ident.as_ref().unwrap();
            if processed.fields_with_setters.contains(&field_name.to_string()) {
                let field_type = &field.ty;
                let setter_name = quote::format_ident!("set_{}", field_name);
                let builder_name = quote::format_ident!("with_{}", field_name);
                
                if let Some(inner_type) = extract_option_type(field_type) {
                    if let Type::Path(type_path) = inner_type {
                        if let Some(ident) = type_path.path.get_ident() {
                            let type_name = ident.to_string();
                            if type_name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) 
                                && !is_standard_type(&type_name) {
                                setter_methods.push(quote! {
                                    pub fn #setter_name(&mut self, value: &#inner_type) {
                                        self.#field_name = Some(value.clone());
                                    }
                                });
                                builder_methods.push(quote! {
                                    pub fn #builder_name(mut self, value: &#inner_type) -> Self {
                                        self.#field_name = Some(value.clone());
                                        self
                                    }
                                });
                                continue;
                            }
                        }
                    }
                }
                
                setter_methods.push(quote! {
                    pub fn #setter_name(&mut self, value: #field_type) {
                        self.#field_name = value;
                    }
                });
                builder_methods.push(quote! {
                    pub fn #builder_name(mut self, value: #field_type) -> Self {
                        self.#field_name = value;
                        self
                    }
                });
            }
        }
    }
    
    if !setter_methods.is_empty() || !builder_methods.is_empty() {
        quote! {
            impl #struct_name {
                #(#setter_methods)*
                #(#builder_methods)*
            }
        }
    } else {
        quote! {}
    }
}

fn generate_default_impl(
    struct_name: &syn::Ident,
    processed: &ProcessedStruct,
) -> proc_macro2::TokenStream {
    if processed.field_defaults.is_empty() {
        return quote! {};
    }
    
    let mut field_initializers = Vec::new();
    
    if let Fields::Named(fields) = &processed.modified_struct.fields {
        for field in &fields.named {
            let field_name = field.ident.as_ref().unwrap();
            let field_init = if let Some(default_expr) = processed.field_defaults.get(&field_name.to_string()) {
                quote! { #field_name: #default_expr }
            } else {
                quote! { #field_name: Default::default() }
            };
            field_initializers.push(field_init);
        }
    }
    
    quote! {
        impl Default for #struct_name {
            fn default() -> Self {
                Self {
                    #(#field_initializers),*
                }
            }
        }
    }
}

// ===== Type Wrapper Generation =====

fn extract_object_methods(
    file: &syn::File,
    type_name: &str,
) -> Option<Vec<(String, Vec<String>, Vec<Type>, Type, ReceiverType)>> {
    use syn::{Item, FnArg, Pat, ReturnType};

    for item in &file.items {
        if let Item::Macro(item_macro) = item {
            let is_object_macro = item_macro.mac.path.is_ident("object")
                || (item_macro.mac.path.segments.len() == 2
                    && item_macro.mac.path.segments[0].ident == "hotline"
                    && item_macro.mac.path.segments[1].ident == "object");

            if is_object_macro {
                if let Ok(obj_input) = syn::parse2::<ObjectInput>(item_macro.mac.tokens.clone()) {
                    let mut methods = Vec::new();
                    
                    // Process fields for getters and builders
                    if let Fields::Named(fields) = &obj_input.struct_item.fields {
                        for field in &fields.named {
                            let field_name = field.ident.as_ref().unwrap();
                            let field_type = &field.ty;
                            
                            let is_public = matches!(field.vis, syn::Visibility::Public(_));
                            let has_setter = field.attrs.iter().any(|attr| attr.path().is_ident("setter"));
                            
                            if is_public && !is_generic_type(field_type) {
                                methods.push((
                                    field_name.to_string(),
                                    vec![],
                                    vec![],
                                    field_type.clone(),
                                    ReceiverType::RefMut,
                                ));
                            }
                            
                            if has_setter {
                                let builder_method_name = format!("with_{}", field_name);
                                let return_type: Type = syn::parse_str(type_name).expect("Failed to parse type name");
                                
                                if let Some(inner_type) = extract_option_type(field_type) {
                                    if let Type::Path(type_path) = inner_type {
                                        if let Some(ident) = type_path.path.get_ident() {
                                            let inner_type_name = ident.to_string();
                                            if is_object_type(&inner_type_name) {
                                                let ref_type: Type = syn::parse_quote! { &#inner_type };
                                                methods.push((
                                                    builder_method_name,
                                                    vec![field_name.to_string()],
                                                    vec![ref_type],
                                                    return_type,
                                                    ReceiverType::Value,
                                                ));
                                                continue;
                                            }
                                        }
                                    }
                                }
                                
                                methods.push((
                                    builder_method_name,
                                    vec![field_name.to_string()],
                                    vec![field_type.clone()],
                                    return_type,
                                    ReceiverType::Value,
                                ));
                            }
                        }
                    }
                    
                    // Find main impl block
                    let main_impl = obj_input.impl_blocks.iter().find(|impl_block| {
                        impl_block.trait_.is_none() && 
                        if let Type::Path(type_path) = &*impl_block.self_ty {
                            type_path.path.get_ident().map(|i| i.to_string()) == Some(type_name.to_string())
                        } else {
                            false
                        }
                    });
                    
                    if let Some(main_impl) = main_impl {
                        for impl_item in &main_impl.items {
                            if let ImplItem::Fn(method) = impl_item {
                                if matches!(method.vis, syn::Visibility::Public(_)) {
                                    let method_name = method.sig.ident.to_string();

                                    let receiver_type = if let Some(FnArg::Receiver(receiver)) = method.sig.inputs.first() {
                                        if receiver.reference.is_none() {
                                            ReceiverType::Value
                                        } else if receiver.mutability.is_some() {
                                            ReceiverType::RefMut
                                        } else {
                                            ReceiverType::Ref
                                        }
                                    } else {
                                        continue;
                                    };

                                    let mut param_names = Vec::new();
                                    let mut param_types = Vec::new();

                                    for arg in method.sig.inputs.iter().skip(1) {
                                        if let FnArg::Typed(typed) = arg {
                                            if let Pat::Ident(pat_ident) = &*typed.pat {
                                                param_names.push(pat_ident.ident.to_string());
                                                param_types.push((*typed.ty).clone());
                                            }
                                        }
                                    }

                                    let return_type = match &method.sig.output {
                                        ReturnType::Default => syn::parse_quote! { () },
                                        ReturnType::Type(_, ty) => resolve_self_type((**ty).clone(), type_name),
                                    };

                                    methods.push((method_name, param_names, param_types, return_type, receiver_type));
                                }
                            }
                        }
                    }

                    return Some(methods);
                }
            }
        }
    }
    None
}

fn generate_typed_wrapper(
    type_name: &str,
    methods: &[(String, Vec<String>, Vec<Type>, Type, ReceiverType)],
    rustc_commit: &str,
) -> proc_macro2::TokenStream {
    let type_ident = quote::format_ident!("{}", type_name);
    
    // Base wrapper struct
    let wrapper_struct = quote! {
        #[derive(Clone)]
        pub struct #type_ident(::hotline::ObjectHandle);

        impl #type_ident {
            pub fn new() -> Self {
                let obj = with_library_registry(|registry| {
                    registry.call_constructor(concat!("lib", #type_name), #type_name, ::hotline::RUSTC_COMMIT)
                        .expect(&format!("failed to construct {}", #type_name))
                }).expect("library registry not initialized");
                
                Self::from_handle(::std::sync::Arc::new(::std::sync::Mutex::new(obj)))
            }

            pub fn from_handle(handle: ::hotline::ObjectHandle) -> Self {
                Self(handle)
            }

            pub fn handle(&self) -> &::hotline::ObjectHandle {
                &self.0
            }
        }

        impl ::std::ops::Deref for #type_ident {
            type Target = ::hotline::ObjectHandle;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl ::std::ops::DerefMut for #type_ident {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }
    };
    
    // Method implementations
    let mut method_impls = Vec::new();
    
    for (method_name, param_names, param_types, return_type, receiver_type) in methods {
        let method_ident = quote::format_ident!("{}", method_name);
        let param_idents: Vec<_> = param_names.iter()
            .map(|name| quote::format_ident!("{}", name))
            .collect();
        
        let returns_self = if let Type::Path(type_path) = return_type {
            type_path.path.is_ident(type_name)
        } else {
            false
        };
        
        // Skip methods that take self by value and don't return Self
        if *receiver_type == ReceiverType::Value && !returns_self {
            continue;
        }
        
        // Build symbol name
        let param_specs: Vec<_> = param_names.iter()
            .zip(param_types.iter())
            .map(|(name, ty)| (name.clone(), type_to_string(ty)))
            .collect();
        
        let symbol = SymbolName::new(type_name, method_name, rustc_commit)
            .with_params(param_specs)
            .with_return_type(type_to_string(return_type));
        
        // Builder pattern methods
        if *receiver_type == ReceiverType::Value && returns_self {
            let setter_method_name = if method_name.starts_with("with_") {
                format!("set_{}", &method_name[5..])
            } else {
                String::new()
            };
            
            let needs_option_wrap = method_name.starts_with("with_") && 
                param_types.len() == 1 && {
                    let inner_type = if let Type::Reference(type_ref) = &param_types[0] {
                        &*type_ref.elem
                    } else {
                        &param_types[0]
                    };
                    
                    if let Type::Path(type_path) = inner_type {
                        if let Some(ident) = type_path.path.get_ident() {
                            let type_name = ident.to_string();
                            type_name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) 
                                && !is_standard_type(&type_name)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };
            
            if needs_option_wrap {
                let inner_type_str = if let Type::Reference(type_ref) = &param_types[0] {
                    type_to_string(&*type_ref.elem)
                } else {
                    type_to_string(&param_types[0])
                };
                
                method_impls.push(quote! {
                    pub fn #method_ident(mut self #(, #param_idents: #param_types)*) -> Self {
                        with_library_registry(|registry| {
                            if let Ok(mut guard) = self.0.lock() {
                                let type_name = guard.type_name();
                                
                                let symbol_name = format!(
                                    "{}__{}______obj_mut_dyn_Any____value__ref_{}____to__unit__{}",
                                    type_name,
                                    #setter_method_name,
                                    #inner_type_str,
                                    #rustc_commit
                                );
                                let lib_name = format!("lib{}", type_name);
                                
                                let obj_any = guard.as_any_mut();
                                
                                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any, #(#param_types),*);
                                registry.with_symbol::<FnType, _, _>(
                                    &lib_name,
                                    &symbol_name,
                                    |fn_ptr| unsafe { (**fn_ptr)(obj_any, #(#param_idents),*) }
                                ).unwrap_or_else(|e| {
                                    panic!("Setter {} not found: {}", #setter_method_name, e);
                                });
                            }
                        });
                        self
                    }
                });
            } else if !setter_method_name.is_empty() && method_name.starts_with("with_") {
                let field_name_str = setter_method_name[4..].to_string();
                let param_type_str = if param_types.len() == 1 {
                    type_to_string(&param_types[0])
                } else {
                    panic!("Field setter should have exactly one parameter")
                };
                
                method_impls.push(quote! {
                    pub fn #method_ident(mut self #(, #param_idents: #param_types)*) -> Self {
                        with_library_registry(|registry| {
                            if let Ok(mut guard) = self.0.lock() {
                                let type_name = guard.type_name();
                                
                                let symbol_name = format!(
                                    "{}__set_{}____obj_mut_dyn_Any__{}_{}__to__unit__{}",
                                    type_name,
                                    #field_name_str,
                                    #field_name_str,
                                    #param_type_str,
                                    #rustc_commit
                                );
                                let lib_name = format!("lib{}", type_name);
                                
                                let obj_any = guard.as_any_mut();
                                
                                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any, #(#param_types)*);
                                registry.with_symbol::<FnType, _, _>(
                                    &lib_name,
                                    &symbol_name,
                                    |fn_ptr| unsafe { (**fn_ptr)(obj_any, #(#param_idents)*) }
                                ).unwrap_or_else(|e| {
                                    panic!("Field setter for {} not found: {}", #field_name_str, e);
                                });
                            }
                        });
                        self
                    }
                });
            } else {
                method_impls.push(quote! {
                    pub fn #method_ident(self #(, #param_idents: #param_types)*) -> Self {
                        self
                    }
                });
            }
        } else {
            // Regular methods
            let symbol_name = symbol.build_method();
            let fn_type = quote! {
                unsafe extern "Rust" fn(&mut dyn std::any::Any #(, #param_types)*) -> #return_type
            };
            
            if returns_self {
                method_impls.push(quote! {
                    pub fn #method_ident(&mut self #(, #param_idents: #param_types)*) -> Self {
                        with_library_registry(|registry| {
                            if let Ok(mut guard) = self.0.lock() {
                                let type_name = guard.type_name();
                                let lib_name = format!("lib{}", type_name);
                                let obj_any = guard.as_any_mut();

                                type FnType = #fn_type;
                                let result = registry.with_symbol::<FnType, _, _>(
                                    &lib_name,
                                    &#symbol_name,
                                    |fn_ptr| unsafe { (**fn_ptr)(obj_any #(, #param_idents)*) }
                                ).unwrap_or_else(|e| panic!("Target object {} doesn't have {} method: {}", type_name, stringify!(#method_ident), e));
                                
                                drop(guard);
                                Self::from_handle(self.0.clone())
                            } else {
                                panic!("Failed to lock object for method {}", stringify!(#method_ident))
                            }
                        }).unwrap_or_else(|| panic!("No library registry available for method {}", stringify!(#method_ident)))
                    }
                });
            } else {
                method_impls.push(quote! {
                    pub fn #method_ident(&mut self #(, #param_idents: #param_types)*) -> #return_type {
                        with_library_registry(|registry| {
                            if let Ok(mut guard) = self.0.lock() {
                                let type_name = guard.type_name();
                                let lib_name = format!("lib{}", type_name);
                                let obj_any = guard.as_any_mut();

                                type FnType = #fn_type;
                                registry.with_symbol::<FnType, _, _>(
                                    &lib_name,
                                    &#symbol_name,
                                    |fn_ptr| unsafe { (**fn_ptr)(obj_any #(, #param_idents)*) }
                                ).unwrap_or_else(|e| panic!("Target object {} doesn't have {} method: {}", type_name, stringify!(#method_ident), e))
                            } else {
                                panic!("Failed to lock object for method {}", stringify!(#method_ident))
                            }
                        }).unwrap_or_else(|| panic!("No library registry available for method {}", stringify!(#method_ident)))
                    }
                });
            }
        }
    }
    
    quote! {
        #wrapper_struct
        
        impl #type_ident {
            #(#method_impls)*
        }
    }
}

fn generate_typed_wrappers(types: &HashSet<String>, rustc_commit: &str) -> proc_macro2::TokenStream {
    let mut wrappers = Vec::new();
    
    for type_name in types {
        let lib_path = find_object_lib_file(type_name);
        if !lib_path.exists() {
            continue;
        }
        
        if let Ok(content) = fs::read_to_string(&lib_path) {
            if let Ok(file) = syn::parse_file(&content) {
                if let Some(methods) = extract_object_methods(&file, type_name) {
                    wrappers.push(generate_typed_wrapper(type_name, &methods, rustc_commit));
                }
            }
        }
    }
    
    quote! { #(#wrappers)* }
}

// ===== Main Macro =====

fn get_rustc_commit_hash() -> String {
    if let Ok(hash) = std::env::var("RUSTC_COMMIT_HASH") {
        return hash;
    }

    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc).arg("-vV").output().expect("Failed to execute rustc");
    let version_info = String::from_utf8(output.stdout).expect("Invalid UTF-8");

    version_info
        .lines()
        .find(|line| line.starts_with("commit-hash: "))
        .and_then(|line| line.strip_prefix("commit-hash: "))
        .map(|hash| hash[..9].to_string())
        .expect("Failed to find rustc commit hash")
}

#[proc_macro]
#[proc_macro_error]
pub fn object(input: TokenStream) -> TokenStream {
    let ObjectInput { struct_item, impl_blocks } = syn::parse_macro_input!(input as ObjectInput);
    let struct_name = &struct_item.ident;
    let rustc_commit = get_rustc_commit_hash();
    
    // Process struct attributes
    let processed = process_struct_attributes(&struct_item);
    
    // Find main impl block
    let (main_impl, other_impl_blocks): (Vec<_>, Vec<_>) = impl_blocks.iter()
        .partition(|impl_block| {
            impl_block.trait_.is_none() && 
            if let Type::Path(type_path) = &*impl_block.self_ty {
                type_path.path.is_ident(struct_name)
            } else {
                false
            }
        });
    
    let main_impl = main_impl.into_iter().next()
        .expect("Expected at least one impl block for the struct itself");
    
    // Check for Default
    let struct_attrs = &struct_item.attrs;
    let has_derive_default = struct_attrs.iter()
        .any(|attr| attr.path().is_ident("derive") && attr.to_token_stream().to_string().contains("Default"));
    
    let has_impl_default = other_impl_blocks.iter().any(|impl_block| {
        if let Some((_, trait_path, _)) = &impl_block.trait_ {
            trait_path.segments.last().map(|seg| seg.ident == "Default").unwrap_or(false)
        } else {
            false
        }
    });
    
    let should_generate_default = !has_derive_default && !has_impl_default && !processed.field_defaults.is_empty();
    let has_default = has_derive_default || has_impl_default || !processed.field_defaults.is_empty();
    
    // Filter out manual Default if we're generating one
    let filtered_other_impl_blocks: Vec<_> = if should_generate_default {
        other_impl_blocks.into_iter().filter(|impl_block| {
            if let Some((_, trait_path, _)) = &impl_block.trait_ {
                !trait_path.segments.last().map(|seg| seg.ident == "Default").unwrap_or(false)
            } else {
                true
            }
        }).collect()
    } else {
        other_impl_blocks
    };
    
    // Generate components
    let field_accessors = generate_field_accessors(struct_name, &processed, &rustc_commit);
    let method_wrappers = generate_method_wrappers(struct_name, main_impl, &processed, &rustc_commit);
    let core_functions = generate_core_functions(struct_name, &rustc_commit, has_default);
    let setter_builder_impl = generate_setter_builder_methods(struct_name, &processed);
    let default_impl = if should_generate_default {
        generate_default_impl(struct_name, &processed)
    } else {
        quote! {}
    };
    
    // HotlineObject trait implementation
    let trait_impl = quote! {
        impl ::hotline::HotlineObject for #struct_name {
            fn type_name(&self) -> &'static str {
                stringify!(#struct_name)
            }

            fn as_any(&self) -> &dyn ::std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn ::std::any::Any {
                self
            }
        }
    };
    
    // Find referenced types and generate wrappers
    let referenced_types = find_referenced_object_types(&struct_item, &impl_blocks);
    let typed_wrappers = generate_typed_wrappers(&referenced_types, &rustc_commit);
    
    // Assemble output
    let modified_struct = &processed.modified_struct;
    let output = quote! {
        #[allow(dead_code)]
        type Like<T> = T;
        
        #modified_struct
        #main_impl
        #(#filtered_other_impl_blocks)*
        #default_impl
        #trait_impl
        #setter_builder_impl
        #(#field_accessors)*
        #(#method_wrappers)*
        #core_functions
        #typed_wrappers
    };
    
    TokenStream::from(output)
}