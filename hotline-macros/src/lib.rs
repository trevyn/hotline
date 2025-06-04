use proc_macro::TokenStream;
use proc_macro_error2::proc_macro_error;
use quote::{ToTokens, quote};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;
use syn::{
    Fields, FnArg, GenericArgument, ImplItem, ItemImpl, ItemStruct, Pat, PathArguments, ReturnType, Type, braced,
    parse::{Parse, ParseStream},
};

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
        
        // Parse all remaining impl blocks
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

fn find_referenced_object_types(struct_item: &ItemStruct, impl_blocks: &[ItemImpl]) -> HashSet<String> {
    use syn::visit::{self, Visit};

    struct TypeVisitor {
        types: HashSet<String>,
        current_type: String,
    }

    impl<'ast> Visit<'ast> for TypeVisitor {
        fn visit_type(&mut self, ty: &'ast Type) {
            // Check for Like<T> pattern
            if let Some(inner_ty) = extract_like_type(ty) {
                // Recursively visit the inner type
                self.visit_type(inner_ty);
                return;
            }
            
            if let Type::Path(type_path) = ty {
                if let Some(ident) = type_path.path.get_ident() {
                    let name = ident.to_string();
                    // Check if this looks like an object type (capitalized, not a standard type)
                    if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                        && name != self.current_type
                        && name != "Self"
                        && !is_standard_type(&name)
                    {
                        self.types.insert(name);
                    }
                }
            }
            visit::visit_type(self, ty);
        }

        fn visit_expr(&mut self, expr: &'ast syn::Expr) {
            // Look for string literals that might be object names (e.g., create_object("Rect"))
            if let syn::Expr::Lit(expr_lit) = expr {
                if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                    let value = lit_str.value();
                    if value.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) && value != self.current_type {
                        self.types.insert(value);
                    }
                }
            }
            
            // Look for method calls like ObjectType::new()
            if let syn::Expr::Call(expr_call) = expr {
                if let syn::Expr::Path(path_expr) = &*expr_call.func {
                    if path_expr.path.segments.len() == 2 {
                        let type_name = path_expr.path.segments[0].ident.to_string();
                        let method_name = path_expr.path.segments[1].ident.to_string();
                        if method_name == "new" && type_name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) 
                            && type_name != self.current_type && !is_standard_type(&type_name) {
                            self.types.insert(type_name);
                        }
                    }
                }
            }
            
            visit::visit_expr(self, expr);
        }
    }

    let mut visitor = TypeVisitor { types: HashSet::new(), current_type: struct_item.ident.to_string() };

    // Visit struct fields
    visitor.visit_item_struct(struct_item);

    // Visit all impl blocks
    for impl_block in impl_blocks {
        visitor.visit_item_impl(impl_block);
    }

    visitor.types
}

fn is_standard_type(name: &str) -> bool {
    matches!(
        name,
        "String"
            | "Vec"
            | "Option"
            | "Result"
            | "Box"
            | "Arc"
            | "Mutex"
            | "HashMap"
            | "BTreeMap"
            | "HashSet"
            | "BTreeSet"
            | "Cell"
            | "RefCell"
            | "Rc"
            | "Weak"
            | "PhantomData"
            | "Pin"
            | "Future"
            | "Stream"
    )
}

fn extract_option_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
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

fn extract_like_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Like" {
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

fn generate_typed_wrappers(types: &HashSet<String>, rustc_commit: &str) -> proc_macro2::TokenStream {
    if types.is_empty() {
        return quote! {};
    }

    let mut wrapper_code = Vec::new();

    for type_name in types {
        // Check if the object actually exists
        let lib_path = find_object_lib_file(type_name);
        if !lib_path.exists() {
            continue;
        }

        let type_ident = quote::format_ident!("{}", type_name);

        // Generate the typed wrapper as a local type alias/newtype
        wrapper_code.push(quote! {
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
        });

        // Read and parse the object's methods
        if let Ok(content) = fs::read_to_string(&lib_path) {
            if let Ok(file) = syn::parse_file(&content) {
                if let Some(methods) = extract_object_methods_for_wrapper(&file, type_name) {
                    wrapper_code.push(generate_methods_for_type(type_name, &methods, rustc_commit));
                }
            }
        }
    }

    quote! {
        #(#wrapper_code)*
    }
}

#[derive(Debug, Clone, PartialEq)]
enum ReceiverType {
    Value,      // self
    Ref,        // &self
    RefMut,     // &mut self
}

fn extract_object_methods_for_wrapper(
    file: &syn::File,
    type_name: &str,
) -> Option<Vec<(String, Vec<String>, Vec<Type>, Type, ReceiverType)>> {
    use syn::{FnArg, ImplItem, Item, Pat, ReturnType};

    for item in &file.items {
        if let Item::Macro(item_macro) = item {
            let is_object_macro = item_macro.mac.path.is_ident("object")
                || (item_macro.mac.path.segments.len() == 2
                    && item_macro.mac.path.segments[0].ident == "hotline"
                    && item_macro.mac.path.segments[1].ident == "object");

            if is_object_macro {
                if let Ok(obj_input) = syn::parse2::<ObjectInput>(item_macro.mac.tokens.clone()) {
                    let mut methods = Vec::new();
                    
                    // Find fields with #[setter] to generate builder methods
                    if let Fields::Named(fields) = &obj_input.struct_item.fields {
                        for field in &fields.named {
                            let field_name = field.ident.as_ref().unwrap();
                            let field_type = &field.ty;
                            
                            // Check if field is public (for getter generation)
                            let is_public = matches!(field.vis, syn::Visibility::Public(_));
                            
                            // Generate getter methods for public fields
                            if is_public && !is_generic_type(field_type) {
                                let getter_method_name = field_name.to_string();
                                methods.push((
                                    getter_method_name,
                                    vec![],
                                    vec![],
                                    field_type.clone(),
                                    ReceiverType::RefMut,
                                ));
                            }
                            
                            // Check if field has #[setter] attribute
                            let has_setter = field.attrs.iter().any(|attr| attr.path().is_ident("setter"));
                            
                            if has_setter {
                                // Add builder method (with_fieldname)
                                let builder_method_name = format!("with_{}", field_name);
                                let return_type: Type = syn::parse_str(type_name).expect("Failed to parse type name");
                                
                                // Check if the field type is Option<T> where T is a clonable object type
                                if let Some(inner_type) = extract_option_type(field_type) {
                                    // Check if inner type looks like an object type (capitalized)
                                    if let Type::Path(type_path) = inner_type {
                                        if let Some(ident) = type_path.path.get_ident() {
                                            let inner_type_name = ident.to_string();
                                            if inner_type_name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) 
                                                && !is_standard_type(&inner_type_name) {
                                                // For Option<ObjectType>, the wrapper method should take &ObjectType
                                                let param_names = vec![field_name.to_string()];
                                                let param_types = vec![inner_type.clone()];
                                                
                                                methods.push((
                                                    builder_method_name,
                                                    param_names,
                                                    param_types,
                                                    return_type,
                                                    ReceiverType::Value,
                                                ));
                                                continue;
                                            }
                                        }
                                    }
                                }
                                
                                // Default: use the full field type
                                let param_names = vec![field_name.to_string()];
                                let param_types = vec![field_type.clone()];
                                
                                methods.push((
                                    builder_method_name,
                                    param_names,
                                    param_types,
                                    return_type,
                                    ReceiverType::Value,
                                ));
                            }
                        }
                    }
                    
                    // Find the main impl block
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

                                // Determine receiver type
                                let receiver_type = if let Some(FnArg::Receiver(receiver)) = method.sig.inputs.first() {
                                    if receiver.reference.is_none() {
                                        ReceiverType::Value
                                    } else if receiver.mutability.is_some() {
                                        ReceiverType::RefMut
                                    } else {
                                        ReceiverType::Ref
                                    }
                                } else {
                                    // No self receiver, skip this method
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
                                    ReturnType::Type(_, ty) => {
                                        // Replace Self with the actual type name
                                        resolve_self_type((**ty).clone(), type_name)
                                    },
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

fn generate_methods_for_type(
    type_name: &str,
    methods: &[(String, Vec<String>, Vec<Type>, Type, ReceiverType)],
    rustc_commit: &str,
) -> proc_macro2::TokenStream {
    let type_ident = quote::format_ident!("{}", type_name);
    let mut impl_methods = Vec::new();

    for (method_name, param_names, param_types, return_type, receiver_type) in methods {
        let method_ident = quote::format_ident!("{}", method_name);
        let param_idents: Vec<proc_macro2::TokenStream> = param_names
            .iter()
            .map(|name| {
                let ident = quote::format_ident!("{}", name);
                quote! { #ident }
            })
            .collect();

        let fn_type = quote! {
            unsafe extern "Rust" fn(&mut dyn std::any::Any #(, #param_types)*) -> #return_type
        };

        // Build symbol name
        let params_part = if param_names.is_empty() {
            String::new()
        } else {
            let param_parts: Vec<String> = param_names
                .iter()
                .zip(param_types.iter())
                .map(|(name, ty)| {
                    let type_str = type_to_string(ty);
                    format!("____{}__{}", name, type_str)
                })
                .collect();
            param_parts.join("")
        };

        let return_part = type_to_string(return_type);

        // Check if return type is Self (now resolved to the actual type name)
        let returns_self = if let Type::Path(type_path) = return_type {
            type_path.path.is_ident(type_name)
        } else {
            false
        };

        // For builder pattern methods (self by value, returns Self)
        if *receiver_type == ReceiverType::Value && returns_self {
            // Generate builder pattern method
            // Try to find if there's a corresponding setter method (with_ -> set_)
            let setter_method_name = if method_name.starts_with("with_") {
                format!("set_{}", &method_name[5..])
            } else {
                // For other builder methods, we can't easily determine the setter
                String::new()
            };
            
            // For builder methods (with_X), we assume there's always a corresponding setter
            // since the macro generates setters for all fields with #[setter]
            let has_setter = !setter_method_name.is_empty() && method_name.starts_with("with_");
            
            let method_impl = if has_setter {
                // For field setters, we need the mangled type string
                // We know there's exactly one parameter for field setters
                let param_type_str = if param_types.len() == 1 {
                    type_to_string(&param_types[0])
                } else {
                    panic!("Field setter should have exactly one parameter")
                };
                
                // Extract field name from setter_method_name (set_X -> X)  
                let field_name_str = setter_method_name[4..].to_string(); // Remove "set_" prefix
                
                quote! {
                    pub fn #method_ident(mut self #(, #param_idents: #param_types)*) -> Self {
                        // Call the field setter via FFI
                        with_library_registry(|registry| {
                            if let Ok(mut guard) = self.0.lock() {
                                let type_name = guard.type_name();
                                
                                // Build the field setter symbol name
                                // Format: {struct}__set_{field}____obj_mut_dyn_Any__{field}_{type}__to__unit__{commit}
                                // Note: single underscore between field and type to match actual symbol
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
                }
            } else {
                // Check if this is a builder method for Option<T> where T is an object type
                // In that case, we expect param_types to have a single type T (not Option<T>)
                // and we need to wrap it in Some()
                let needs_option_wrap = method_name.starts_with("with_") && 
                    param_types.len() == 1 && 
                    if let Type::Path(type_path) = &param_types[0] {
                        if let Some(ident) = type_path.path.get_ident() {
                            let type_name = ident.to_string();
                            type_name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) 
                                && !is_standard_type(&type_name)
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                
                if needs_option_wrap {
                    // This is a builder method for Option<ObjectType>
                    // Call the corresponding setter method via FFI
                    let setter_method_name = format!("set_{}", &method_name[5..]); // with_X -> set_X
                    quote! {
                        pub fn #method_ident(mut self #(, #param_idents: &#param_types)*) -> Self {
                            // Call the setter via FFI
                            with_library_registry(|registry| {
                                if let Ok(mut guard) = self.0.lock() {
                                    let type_name = guard.type_name();
                                    
                                    // Build the setter symbol name
                                    let param_type_str = #(stringify!(#param_types)),*;
                                    let symbol_name = format!(
                                        "{}__{}______obj_mut_dyn_Any____value__ref_{}____to__unit__{}",
                                        type_name,
                                        #setter_method_name,
                                        param_type_str,
                                        #rustc_commit
                                    );
                                    let lib_name = format!("lib{}", type_name);
                                    
                                    let obj_any = guard.as_any_mut();
                                    
                                    type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any, &#(#param_types),*);
                                    registry.with_symbol::<FnType, _, _>(
                                        &lib_name,
                                        &symbol_name,
                                        |fn_ptr| unsafe { (**fn_ptr)(obj_any, #(#param_idents),*) }
                                    ).unwrap_or_else(|e| {
                                        // Setter doesn't exist, log but don't panic
                                        eprintln!("Warning: setter {} not found: {}", #setter_method_name, e);
                                    });
                                }
                            });
                            self
                        }
                    }
                } else {
                    // Fallback: just return self without modification
                    quote! {
                        pub fn #method_ident(self #(, #param_idents: #param_types)*) -> Self {
                            // Builder method without corresponding setter - returning self unchanged
                            // The actual object implementation handles the state change internally
                            self
                        }
                    }
                }
            };
            
            impl_methods.push(method_impl);
            continue;
        }

        // For regular methods, ensure we're not trying to handle self by value
        if *receiver_type == ReceiverType::Value {
            // Skip methods that take self by value but don't return Self
            // These can't be wrapped via FFI
            continue;
        }

        let method_impl = if returns_self {
            // For methods that return Self, return the wrapper type
            quote! {
                pub fn #method_ident(&mut self #(, #param_idents: #param_types)*) -> Self {
                    with_library_registry(|registry| {
                        if let Ok(mut guard) = self.0.lock() {
                            let type_name = guard.type_name();

                            let symbol_name = format!(
                                "{}__{}______obj_mut_dyn_Any{}____to__{}__{}",
                                type_name,
                                stringify!(#method_ident),
                                #params_part,
                                #return_part,
                                #rustc_commit
                            );
                            let lib_name = format!("lib{}", type_name);

                            let obj_any = guard.as_any_mut();

                            type FnType = #fn_type;
                            let result = registry.with_symbol::<FnType, _, _>(
                                &lib_name,
                                &symbol_name,
                                |fn_ptr| unsafe { (**fn_ptr)(obj_any #(, #param_idents)*) }
                            ).unwrap_or_else(|e| panic!("Target object {} doesn't have {} method: {}", type_name, stringify!(#method_ident), e));
                            
                            // Wrap the returned object in a new handle
                            drop(guard);
                            Self::from_handle(self.0.clone())
                        } else {
                            panic!("Failed to lock object for method {}", stringify!(#method_ident))
                        }
                    }).unwrap_or_else(|| panic!("No library registry available for method {}", stringify!(#method_ident)))
                }
            }
        } else {
            quote! {
                pub fn #method_ident(&mut self #(, #param_idents: #param_types)*) -> #return_type {
                    with_library_registry(|registry| {
                        if let Ok(mut guard) = self.0.lock() {
                            let type_name = guard.type_name();

                            let symbol_name = format!(
                                "{}__{}______obj_mut_dyn_Any{}____to__{}__{}",
                                type_name,
                                stringify!(#method_ident),
                                #params_part,
                                #return_part,
                                #rustc_commit
                            );
                            let lib_name = format!("lib{}", type_name);

                            let obj_any = guard.as_any_mut();

                            type FnType = #fn_type;
                            registry.with_symbol::<FnType, _, _>(
                                &lib_name,
                                &symbol_name,
                                |fn_ptr| unsafe { (**fn_ptr)(obj_any #(, #param_idents)*) }
                            ).unwrap_or_else(|e| panic!("Target object {} doesn't have {} method: {}", type_name, stringify!(#method_ident), e))
                        } else {
                            panic!("Failed to lock object for method {}", stringify!(#method_ident))
                        }
                    }).unwrap_or_else(|| panic!("No library registry available for method {}", stringify!(#method_ident)))
                }
            }
        };

        impl_methods.push(method_impl);
    }

    quote! {
        impl #type_ident {
            #(#impl_methods)*
        }
    }
}

#[proc_macro]
#[proc_macro_error]
pub fn object(input: TokenStream) -> TokenStream {
    let ObjectInput { struct_item, impl_blocks } = syn::parse_macro_input!(input as ObjectInput);

    let struct_name = &struct_item.ident;
    let struct_attrs = &struct_item.attrs;

    // Get rustc commit hash at macro expansion time
    let rustc_commit = get_rustc_commit_hash();
    
    // Find the main impl block (impl StructName) and other impl blocks
    let mut main_impl_block = None;
    let mut other_impl_blocks = Vec::new();
    
    for impl_block in &impl_blocks {
        // Check if this is impl StructName (not impl Trait for StructName)
        if impl_block.trait_.is_none() {
            if let Type::Path(type_path) = &*impl_block.self_ty {
                if type_path.path.is_ident(struct_name) {
                    main_impl_block = Some(impl_block);
                } else {
                    other_impl_blocks.push(impl_block);
                }
            } else {
                other_impl_blocks.push(impl_block);
            }
        } else {
            other_impl_blocks.push(impl_block);
        }
    }
    
    let main_impl = main_impl_block.expect("Expected at least one impl block for the struct itself");

    // Generate field accessors
    let mut field_accessors = Vec::new();

    // First, we need to process the struct fields to filter out #[setter] and #[default] attributes
    // and remember which fields have them
    let mut fields_with_setters = HashSet::new();
    let mut field_defaults = std::collections::HashMap::new();
    
    // Create a modified struct with filtered attributes
    let mut modified_struct = struct_item.clone();
    
    if let Fields::Named(fields) = &mut modified_struct.fields {
        for field in &mut fields.named {
            // Check for #[setter] and #[default] attributes
            let mut has_setter = false;
            let mut default_value = None;
            
            field.attrs.retain(|attr| {
                if attr.path().is_ident("setter") {
                    has_setter = true;
                    false // Remove the attribute
                } else if attr.path().is_ident("default") {
                    // Parse the default value
                    if let Ok(value) = attr.parse_args::<syn::Expr>() {
                        default_value = Some(value);
                    }
                    false // Remove the attribute
                } else {
                    true // Keep other attributes
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

    if let Fields::Named(fields) = &modified_struct.fields {
        for field in &fields.named {
            let field_name = field.ident.as_ref().unwrap();
            let field_type = &field.ty;
            let is_public = matches!(field.vis, syn::Visibility::Public(_));

            // Only generate accessors for simple types
            if !is_generic_type(field_type) {
                let type_str = type_to_string(field_type);

                // Getter (only generated for public fields)
                if is_public {
                    let getter_fn_name = quote::format_ident!(
                        "{}__{}______obj_mut_dyn_Any____to__{}__{}",
                        struct_name,
                        field_name,
                        type_str,
                        rustc_commit
                    );

                    field_accessors.push(quote! {
                        #[unsafe(no_mangle)]
                        #[allow(non_snake_case)]
                        pub extern "Rust" fn #getter_fn_name(obj: &mut dyn ::std::any::Any) -> #field_type {
                            let instance = match obj.downcast_mut::<#struct_name>() {
                                Some(inst) => inst,
                                None => {
                                    let type_name: &'static str = ::std::any::type_name_of_val(&*obj);
                                    panic!(
                                        "Type mismatch in getter {}: expected {}, but got {}",
                                        stringify!(#field_name),
                                        stringify!(#struct_name),
                                        type_name
                                    )
                                }
                            };
                            instance.#field_name.clone()
                        }
                    });
                }

                // Setter (generated for any field with #[setter] attribute, regardless of visibility)
                if fields_with_setters.contains(&field_name.to_string()) {
                    let setter_fn_name = quote::format_ident!(
                        "{}__set_{}____obj_mut_dyn_Any__{}_{}__to__unit__{}",
                        struct_name,
                        field_name,
                        field_name,
                        type_str,
                        rustc_commit
                    );

                    field_accessors.push(quote! {
                        #[unsafe(no_mangle)]
                        #[allow(non_snake_case)]
                        pub extern "Rust" fn #setter_fn_name(obj: &mut dyn ::std::any::Any, value: #field_type) {
                            let instance = match obj.downcast_mut::<#struct_name>() {
                                Some(inst) => inst,
                                None => {
                                    let type_name: &'static str = ::std::any::type_name_of_val(&*obj);
                                    panic!(
                                        "Type mismatch in setter {}: expected {}, but got {}",
                                        stringify!(#field_name),
                                        stringify!(#struct_name),
                                        type_name
                                    )
                                }
                            };
                            instance.#field_name = value;
                        }
                    });
                }
            }
        }
    }

    // Generate constructor if Default is derived or implemented
    let has_derive_default = struct_attrs
        .iter()
        .any(|attr| attr.path().is_ident("derive") && attr.to_token_stream().to_string().contains("Default"));
    
    let has_impl_default = other_impl_blocks.iter().any(|impl_block| {
        if let Some((_, trait_path, _)) = &impl_block.trait_ {
            trait_path.segments.last().map(|seg| seg.ident == "Default").unwrap_or(false)
        } else {
            false
        }
    });
    
    // If there are field defaults, we should generate a Default impl (but not if derive is used)
    let should_generate_default = !has_derive_default && !has_impl_default && !field_defaults.is_empty();
    let has_default = has_derive_default || has_impl_default || !field_defaults.is_empty();

    let constructor = if has_default {
        let ctor_fn_name =
            quote::format_ident!("{}__new____to__Box_lt_dyn_HotlineObject_gt__{}", struct_name, rustc_commit);
        quote! {
            #[unsafe(no_mangle)]
            #[allow(non_snake_case)]
            pub extern "Rust" fn #ctor_fn_name() -> Box<dyn ::hotline::HotlineObject> {
                Box::new(<#struct_name as Default>::default()) as Box<dyn ::hotline::HotlineObject>
            }
        }
    } else {
        quote! {}
    };

    // Generate method wrappers
    let mut method_wrappers = Vec::new();
    
    // First, generate FFI wrappers for setter methods
    if let Fields::Named(fields) = &modified_struct.fields {
        for field in &fields.named {
            let field_name = field.ident.as_ref().unwrap();
            if fields_with_setters.contains(&field_name.to_string()) {
                let field_type = &field.ty;
                let setter_name = quote::format_ident!("set_{}", field_name);
                
                // Check if the field type is Option<T> where T is a clonable object type
                if let Some(inner_type) = extract_option_type(field_type) {
                    // Check if inner type looks like an object type (capitalized)
                    if let Type::Path(type_path) = inner_type {
                        if let Some(ident) = type_path.path.get_ident() {
                            let type_name = ident.to_string();
                            if type_name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) 
                                && !is_standard_type(&type_name) {
                                // Generate FFI wrapper for setter that takes &T
                                let type_str = type_to_string(inner_type);
                                let wrapper_fn_name = quote::format_ident!(
                                    "{}__{}______obj_mut_dyn_Any____value__ref_{}____to__unit__{}",
                                    struct_name,
                                    setter_name,
                                    type_str,
                                    rustc_commit
                                );
                                
                                method_wrappers.push(quote! {
                                    #[unsafe(no_mangle)]
                                    #[allow(non_snake_case)]
                                    pub extern "Rust" fn #wrapper_fn_name(
                                        obj: &mut dyn ::std::any::Any,
                                        value: &#inner_type
                                    ) {
                                        let instance = match obj.downcast_mut::<#struct_name>() {
                                            Some(inst) => inst,
                                            None => {
                                                let type_name: &'static str = ::std::any::type_name_of_val(&*obj);
                                                panic!(
                                                    "Type mismatch in method {}: expected {}, but got {}",
                                                    stringify!(#setter_name),
                                                    stringify!(#struct_name),
                                                    type_name
                                                )
                                            }
                                        };
                                        instance.#setter_name(value)
                                    }
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    for item in &main_impl.items {
        if let ImplItem::Fn(method) = item {
            // Only generate extern functions for pub methods
            let is_public = matches!(method.vis, syn::Visibility::Public(_));
            if !is_public {
                continue;
            }

            let method_name = &method.sig.ident;
            let method_output = &method.sig.output;

            // Build arg info
            let mut arg_names = Vec::new();
            let mut arg_types = Vec::new();
            let mut symbol_parts =
                vec![struct_name.to_string(), method_name.to_string(), "____obj_mut_dyn_Any".to_string()];

            // Check receiver type to decide if we should generate FFI wrapper
            if let Some(FnArg::Receiver(receiver)) = method.sig.inputs.first() {
                if receiver.reference.is_none() {
                    // Method takes self by value - skip FFI wrapper generation
                    continue;
                }
                // &self or &mut self - we can generate FFI wrapper
            } else {
                // No self receiver, skip
                continue;
            }

            // Process arguments (skip self)
            for arg in method.sig.inputs.iter().skip(1) {
                if let FnArg::Typed(typed) = arg {
                    if let Pat::Ident(pat_ident) = &*typed.pat {
                        let arg_name = &pat_ident.ident;
                        let arg_type = &*typed.ty;
                        let type_str = type_to_string(&arg_type);

                        arg_names.push(arg_name);
                        arg_types.push(arg_type);
                        symbol_parts.push(format!("__{}__{}", arg_name, type_str));
                    }
                }
            }

            // Add return type
            let return_type_str = match method_output {
                ReturnType::Default => "unit".to_string(),
                ReturnType::Type(_, ty) => {
                    // Resolve Self before converting to string
                    let resolved_ty = resolve_self_type((**ty).clone(), &struct_name.to_string());
                    type_to_string(&resolved_ty)
                },
            };
            symbol_parts.push(format!("__to__{}", return_type_str));
            symbol_parts.push(rustc_commit.clone());

            let wrapper_fn_name = quote::format_ident!("{}", symbol_parts.join("__"));

            // Resolve return type for the wrapper function
            let wrapper_return_type = match method_output {
                ReturnType::Default => quote! {},
                ReturnType::Type(arrow, ty) => {
                    let resolved_ty = resolve_self_type((**ty).clone(), &struct_name.to_string());
                    quote! { #arrow #resolved_ty }
                }
            };

            let wrapper = quote! {
                #[unsafe(no_mangle)]
                #[allow(non_snake_case)]
                pub extern "Rust" fn #wrapper_fn_name(
                    obj: &mut dyn ::std::any::Any
                    #(, #arg_names: #arg_types)*
                ) #wrapper_return_type {
                    let instance = match obj.downcast_mut::<#struct_name>() {
                        Some(inst) => inst,
                        None => {
                            let type_name: &'static str = ::std::any::type_name_of_val(&*obj);
                            panic!(
                                "Type mismatch in method {}: expected {}, but got {}",
                                stringify!(#method_name),
                                stringify!(#struct_name),
                                type_name
                            )
                        }
                    };
                    instance.#method_name(#(#arg_names),*)
                }
            };

            method_wrappers.push(wrapper);
        }
    }

    // Generate a type name getter
    let type_name_fn = {
        let type_name_fn_name =
            quote::format_ident!("{}__get_type_name____obj_ref_dyn_Any__to__str__{}", struct_name, rustc_commit);
        quote! {
            #[unsafe(no_mangle)]
            #[allow(non_snake_case)]
            pub extern "Rust" fn #type_name_fn_name(obj: &dyn ::std::any::Any) -> &'static str {
                // Verify it's the right type
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
        }
    };

    // Generate HotlineObject trait implementation
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

    // Generate init function for setting library registry
    let init_fn_name = quote::format_ident!("{}__init__registry__{}", struct_name, rustc_commit);

    let init_function = quote! {
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

    // Generate setter and builder methods for fields with #[setter]
    let mut setter_methods = Vec::new();
    let mut builder_methods = Vec::new();
    if let Fields::Named(fields) = &modified_struct.fields {
        for field in &fields.named {
            let field_name = field.ident.as_ref().unwrap();
            if fields_with_setters.contains(&field_name.to_string()) {
                let field_type = &field.ty;
                let setter_name = quote::format_ident!("set_{}", field_name);
                let builder_name = quote::format_ident!("with_{}", field_name);
                
                // Check if the field type is Option<T> where T is a clonable object type
                if let Some(inner_type) = extract_option_type(field_type) {
                    // Check if inner type looks like an object type (capitalized)
                    if let Type::Path(type_path) = inner_type {
                        if let Some(ident) = type_path.path.get_ident() {
                            let type_name = ident.to_string();
                            if type_name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) 
                                && !is_standard_type(&type_name) {
                                // Generate setter that takes &T and wraps in Some()
                                setter_methods.push(quote! {
                                    pub fn #setter_name(&mut self, value: &#inner_type) {
                                        self.#field_name = Some(value.clone());
                                    }
                                });
                                // Generate builder that takes &T  
                                builder_methods.push(quote! {
                                    pub fn #builder_name(mut self, value: &#inner_type) -> Self {
                                        self.#setter_name(value);
                                        self
                                    }
                                });
                                continue;
                            }
                        }
                    }
                }
                
                // Default behavior for other types
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
    
    let setter_builder_impl = if !builder_methods.is_empty() || !setter_methods.is_empty() {
        quote! {
            impl #struct_name {
                #(#setter_methods)*
                #(#builder_methods)*
            }
        }
    } else {
        quote! {}
    };
    
    // Generate Default implementation if we have field defaults and no manual implementation
    let default_impl = if should_generate_default {
        let mut field_initializers = Vec::new();
        
        if let Fields::Named(fields) = &modified_struct.fields {
            for field in &fields.named {
                let field_name = field.ident.as_ref().unwrap();
                let field_init = if let Some(default_expr) = field_defaults.get(&field_name.to_string()) {
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
    } else {
        quote! {}
    };
    
    // Filter out any manual Default implementations if we're generating one
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

    // Generate output
    let output = quote! {
        #modified_struct

        #main_impl
        
        #(#filtered_other_impl_blocks)*
        
        #default_impl

        #trait_impl
        
        #setter_builder_impl

        #constructor

        #(#field_accessors)*

        #(#method_wrappers)*

        #type_name_fn

        #init_function
    };

    // Find all object types referenced in the code
    let referenced_types = find_referenced_object_types(&struct_item, &impl_blocks);

    // Generate typed wrappers for referenced objects
    let typed_wrappers = generate_typed_wrappers(&referenced_types, &rustc_commit);

    TokenStream::from(quote! {
        // Define Like as a type alias template
        #[allow(dead_code)]
        type Like<T> = T;
        
        #output
        #typed_wrappers
    })
}

fn is_generic_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        for segment in &type_path.path.segments {
            match &segment.arguments {
                PathArguments::AngleBracketed(_) => return true,
                _ => {}
            }
        }
    }
    false
}

fn type_to_string(ty: &Type) -> String {
    // Handle Like<T> specially - just return T's string representation
    if let Some(inner_ty) = extract_like_type(ty) {
        return type_to_string(inner_ty);
    }
    
    match ty {
        Type::Path(type_path) => {
            if let Some(ident) = type_path.path.get_ident() {
                ident.to_string()
            } else {
                // Handle generic types
                let mut result = String::new();
                for (i, segment) in type_path.path.segments.iter().enumerate() {
                    if i > 0 {
                        result.push_str("_");
                    }
                    result.push_str(&segment.ident.to_string());

                    match &segment.arguments {
                        PathArguments::AngleBracketed(args) => {
                            result.push_str("_lt_");
                            for (j, arg) in args.args.iter().enumerate() {
                                if j > 0 {
                                    result.push_str("_");
                                }
                                if let GenericArgument::Type(inner_ty) = arg {
                                    result.push_str(&type_to_string(inner_ty));
                                }
                            }
                            result.push_str("_gt");
                        }
                        _ => {}
                    }
                }
                result
            }
        }
        Type::Reference(type_ref) => {
            let mut result = String::new();
            if type_ref.mutability.is_some() {
                result.push_str("mut_");
            }
            result.push_str("ref_");
            result.push_str(&type_to_string(&type_ref.elem));
            result
        }
        Type::Slice(type_slice) => {
            format!("slice_{}", type_to_string(&type_slice.elem))
        }
        Type::Array(type_array) => {
            format!("array_{}", type_to_string(&type_array.elem))
        }
        Type::Tuple(type_tuple) => {
            if type_tuple.elems.is_empty() {
                "unit".to_string()
            } else {
                let mut result = String::from("tuple_");
                for (i, elem) in type_tuple.elems.iter().enumerate() {
                    if i > 0 {
                        result.push_str("_comma_");
                    }
                    result.push_str(&type_to_string(elem));
                }
                result
            }
        }
        _ => panic!("Unsupported type in hotline macro: {:?}", ty),
    }
}

fn get_rustc_commit_hash() -> String {
    // Try to get from env var first (for when we're building hotline-macros itself)
    if let Ok(hash) = std::env::var("RUSTC_COMMIT_HASH") {
        return hash;
    }

    // Otherwise, get it directly from rustc
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(rustc).arg("-vV").output().expect("Failed to execute rustc");

    let version_info = String::from_utf8(output.stdout).expect("Invalid UTF-8");

    // Extract commit hash (first 9 chars)
    version_info
        .lines()
        .find(|line| line.starts_with("commit-hash: "))
        .and_then(|line| line.strip_prefix("commit-hash: "))
        .map(|hash| hash[..9].to_string())
        .expect("Failed to find rustc commit hash")
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
                    // Replace Self with the actual type name
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

fn resolve_like_type(ty: Type) -> Type {
    use syn::visit_mut::{self, VisitMut};
    
    struct LikeResolver;
    
    impl VisitMut for LikeResolver {
        fn visit_type_mut(&mut self, ty: &mut Type) {
            if let Type::Path(type_path) = ty {
                if type_path.path.segments.len() == 1 {
                    let segment = &type_path.path.segments[0];
                    if segment.ident == "Like" {
                        if let PathArguments::AngleBracketed(args) = &segment.arguments {
                            if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                                // Replace Like<T> with T
                                *ty = inner_ty.clone();
                                return;
                            }
                        }
                    }
                }
            }
            visit_mut::visit_type_mut(self, ty);
        }
    }
    
    let mut ty = ty;
    LikeResolver.visit_type_mut(&mut ty);
    ty
}

fn find_object_lib_file(type_name: &str) -> std::path::PathBuf {
    // Get the workspace root by looking for Cargo.toml
    let mut current_dir = std::env::current_dir().unwrap();
    loop {
        if current_dir.join("Cargo.toml").exists() && current_dir.join("objects").exists() {
            break;
        }
        if !current_dir.pop() {
            // Fallback: try to find objects dir relative to OUT_DIR
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
