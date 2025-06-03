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

    // Generate method wrappers and collect signatures
    let mut method_wrappers = Vec::new();
    let mut method_signatures = Vec::new();

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

            // Collect signature for this method
            let param_types: Vec<String> = arg_types.iter().map(|ty| type_to_string(ty)).collect();
            let return_type = match method_output {
                ReturnType::Default => "()".to_string(),
                ReturnType::Type(_, ty) => ty.to_token_stream().to_string().replace(" ", ""),
            };

            method_signatures.push(format!(
                "{}:{}:{}",
                method_name,
                param_types.join(","),
                return_type
            ));
        }
    }

    // Write signatures to file
    write_signatures(&struct_name.to_string(), &method_signatures);

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

    TokenStream::from(output)
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

fn write_signatures(struct_name: &str, signatures: &[String]) {
    // Get the workspace root by looking for Cargo.toml
    let mut current_dir = std::env::current_dir().unwrap();
    loop {
        if current_dir.join("Cargo.toml").exists() && current_dir.join("signatures").exists() {
            break;
        }
        if !current_dir.pop() {
            // Fallback: try to create signatures next to target dir
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

    let signatures_dir = current_dir.join("signatures");
    fs::create_dir_all(&signatures_dir).ok();

    let sig_file = signatures_dir.join(format!("{}.sig", struct_name));
    let content = signatures.join("\n");
    fs::write(&sig_file, content).ok();
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
