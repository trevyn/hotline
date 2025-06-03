use proc_macro::TokenStream;
use proc_macro_error2::proc_macro_error;
use quote::{ToTokens, quote};
use std::fs;
use std::path::Path;
use std::process::Command;
use syn::{
    Fields, FnArg, GenericArgument, ImplItem, ItemImpl, ItemStruct, Pat, PathArguments, ReturnType,
    Type, braced,
    parse::{Parse, ParseStream},
};


struct ObjectInput {
    struct_item: ItemStruct,
    impl_item: ItemImpl,
}

impl Parse for ObjectInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        braced!(content in input);

        let struct_item: ItemStruct = content
            .parse()
            .map_err(|e| syn::Error::new(e.span(), "Expected a struct definition"))?;
        let impl_item: ItemImpl = content
            .parse()
            .map_err(|e| syn::Error::new(e.span(), "Expected an impl block after the struct"))?;

        Ok(ObjectInput { struct_item, impl_item })
    }
}


fn find_referenced_object_types(struct_item: &ItemStruct, impl_item: &ItemImpl) -> std::collections::HashSet<String> {
    use std::collections::HashSet;
    use syn::visit::{self, Visit};
    
    struct TypeVisitor {
        types: HashSet<String>,
        current_type: String,
    }
    
    impl<'ast> Visit<'ast> for TypeVisitor {
        fn visit_type(&mut self, ty: &'ast Type) {
            if let Type::Path(type_path) = ty {
                if let Some(ident) = type_path.path.get_ident() {
                    let name = ident.to_string();
                    // Check if this looks like an object type (capitalized, not a standard type)
                    if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                        && name != self.current_type
                        && !is_standard_type(&name) {
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
                    if value.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                        && value != self.current_type {
                        self.types.insert(value);
                    }
                }
            }
            visit::visit_expr(self, expr);
        }
    }
    
    let mut visitor = TypeVisitor {
        types: HashSet::new(),
        current_type: struct_item.ident.to_string(),
    };
    
    // Visit struct fields
    visitor.visit_item_struct(struct_item);
    
    // Visit impl methods
    visitor.visit_item_impl(impl_item);
    
    visitor.types
}

fn is_standard_type(name: &str) -> bool {
    matches!(name, "String" | "Vec" | "Option" | "Result" | "Box" | "Arc" | "Mutex" | 
             "HashMap" | "BTreeMap" | "HashSet" | "BTreeSet" | "Cell" | "RefCell" |
             "Rc" | "Weak" | "PhantomData" | "Pin" | "Future" | "Stream")
}

fn generate_typed_wrappers(types: &std::collections::HashSet<String>, rustc_commit: &str) -> proc_macro2::TokenStream {
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

fn extract_object_methods_for_wrapper(file: &syn::File, _type_name: &str) -> Option<Vec<(String, Vec<String>, Vec<Type>, Type)>> {
    use syn::{Item, ImplItem, FnArg, Pat, ReturnType};
    
    for item in &file.items {
        if let Item::Macro(item_macro) = item {
            if item_macro.mac.path.is_ident("object") {
                if let Ok(obj_input) = syn::parse2::<ObjectInput>(item_macro.mac.tokens.clone()) {
                    let mut methods = Vec::new();
                    
                    for impl_item in &obj_input.impl_item.items {
                        if let ImplItem::Fn(method) = impl_item {
                            if matches!(method.vis, syn::Visibility::Public(_)) {
                                let method_name = method.sig.ident.to_string();
                                
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
                                    ReturnType::Type(_, ty) => (**ty).clone(),
                                };
                                
                                methods.push((method_name, param_names, param_types, return_type));
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

fn generate_methods_for_type(type_name: &str, methods: &[(String, Vec<String>, Vec<Type>, Type)], rustc_commit: &str) -> proc_macro2::TokenStream {
    let type_ident = quote::format_ident!("{}", type_name);
    let mut impl_methods = Vec::new();
    
    for (method_name, param_names, param_types, return_type) in methods {
        let method_ident = quote::format_ident!("{}", method_name);
        let param_idents: Vec<proc_macro2::TokenStream> = param_names.iter().map(|name| {
            let ident = quote::format_ident!("{}", name);
            quote! { #ident }
        }).collect();
        
        let fn_type = quote! {
            unsafe extern "Rust" fn(&mut dyn std::any::Any #(, #param_types)*) -> #return_type
        };
        
        // Build symbol name
        let params_part = if param_names.is_empty() {
            String::new()
        } else {
            let param_parts: Vec<String> = param_names.iter().zip(param_types.iter()).map(|(name, ty)| {
                let type_str = type_to_string(ty);
                format!("____{}__{}", name, type_str)
            }).collect();
            param_parts.join("")
        };
        
        let return_part = type_to_string(return_type);
        
        let method_impl = quote! {
            fn #method_ident(&mut self #(, #param_idents: #param_types)*) -> #return_type {
                crate::with_library_registry(|registry| {
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
    let ObjectInput { struct_item, impl_item } = syn::parse_macro_input!(input as ObjectInput);

    let struct_name = &struct_item.ident;
    let struct_attrs = &struct_item.attrs;
    let struct_fields = &struct_item.fields;

    // Get rustc commit hash at macro expansion time
    let rustc_commit = get_rustc_commit_hash();

    // Generate field accessors
    let mut field_accessors = Vec::new();

    if let Fields::Named(fields) = struct_fields {
        for field in &fields.named {
            // Only generate accessors for public fields
            let is_public = matches!(field.vis, syn::Visibility::Public(_));
            if !is_public {
                continue;
            }

            let field_name = field.ident.as_ref().unwrap();
            let field_type = &field.ty;

            // Only generate accessors for simple types
            if !is_generic_type(field_type) {
                let type_str = type_to_string(field_type);

                // Getter
                let getter_fn_name = quote::format_ident!(
                    "{}__get_{}____obj_ref_dyn_Any__to__{}__{}",
                    struct_name,
                    field_name,
                    type_str,
                    rustc_commit
                );

                field_accessors.push(quote! {
                    #[unsafe(no_mangle)]
                    #[allow(non_snake_case)]
                    pub extern "Rust" fn #getter_fn_name(obj: &dyn ::std::any::Any) -> #field_type {
                        let instance = match obj.downcast_ref::<#struct_name>() {
                            Some(inst) => inst,
                            None => {
                                let type_name: &'static str = ::std::any::type_name_of_val(obj);
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

                // Setter
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

    // Generate constructor if Default is derived
    let has_default = struct_attrs.iter().any(|attr| {
        attr.path().is_ident("derive") && attr.to_token_stream().to_string().contains("Default")
    });

    let constructor = if has_default {
        let ctor_fn_name = quote::format_ident!(
            "{}__new____to__Box_lt_dyn_HotlineObject_gt__{}",
            struct_name,
            rustc_commit
        );
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

    for item in &impl_item.items {
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
            let mut symbol_parts = vec![
                struct_name.to_string(),
                method_name.to_string(),
                "____obj_mut_dyn_Any".to_string(),
            ];

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
                ReturnType::Type(_, ty) => type_to_string(ty),
            };
            symbol_parts.push(format!("__to__{}", return_type_str));
            symbol_parts.push(rustc_commit.clone());

            let wrapper_fn_name = quote::format_ident!("{}", symbol_parts.join("__"));

            let wrapper = quote! {
                #[unsafe(no_mangle)]
                #[allow(non_snake_case)]
                pub extern "Rust" fn #wrapper_fn_name(
                    obj: &mut dyn ::std::any::Any
                    #(, #arg_names: #arg_types)*
                ) #method_output {
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
        let type_name_fn_name = quote::format_ident!(
            "{}__get_type_name____obj_ref_dyn_Any__to__str__{}",
            struct_name,
            rustc_commit
        );
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


    // Generate output
    let output = quote! {
        #struct_item

        #impl_item

        #trait_impl

        #constructor

        #(#field_accessors)*

        #(#method_wrappers)*

        #type_name_fn

        #init_function
    };

    // Find all object types referenced in the code
    let referenced_types = find_referenced_object_types(&struct_item, &impl_item);
    
    // Generate typed wrappers for referenced objects
    let typed_wrappers = generate_typed_wrappers(&referenced_types, &rustc_commit);

    TokenStream::from(quote! {
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


