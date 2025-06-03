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
    Token,
};

#[derive(Default, Debug)]
struct UsePrototypes {
    prototypes: Vec<(String, String)>, // (TypeName, method)
}

impl Parse for UsePrototypes {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut prototypes = Vec::new();
        
        // Parse comma-separated list of Type.method
        while !input.is_empty() {
            // Parse Type
            let type_name: syn::Ident = input.parse()?;
            
            // Parse dot
            let _dot: Token![.] = input.parse()?;
            
            // Parse method
            let method_name: syn::Ident = input.parse()?;
            
            prototypes.push((type_name.to_string(), method_name.to_string()));
            
            // Check for comma
            if input.peek(Token![,]) {
                let _comma: Token![,] = input.parse()?;
            } else {
                break;
            }
        }
        
        Ok(UsePrototypes { prototypes })
    }
}

struct ObjectInput {
    use_prototypes: Option<UsePrototypes>,
    struct_item: ItemStruct,
    impl_item: ItemImpl,
}

impl Parse for ObjectInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        braced!(content in input);

        // Try to parse use_prototypes! as a macro invocation
        let use_prototypes = if content.peek(syn::Ident) {
            let fork = content.fork();
            if let Ok(ident) = fork.parse::<syn::Ident>() {
                if ident == "use_prototypes" && fork.peek(Token![!]) {
                    // Consume the tokens we checked
                    let _: syn::Ident = content.parse()?;
                    let _: Token![!] = content.parse()?;
                    
                    // Parse the braced content
                    let proto_content;
                    braced!(proto_content in content);
                    let protos: UsePrototypes = proto_content.parse()?;
                    
                    Some(protos)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let struct_item: ItemStruct = content
            .parse()
            .map_err(|e| syn::Error::new(e.span(), "Expected a struct definition"))?;
        let impl_item: ItemImpl = content
            .parse()
            .map_err(|e| syn::Error::new(e.span(), "Expected an impl block after the struct"))?;

        Ok(ObjectInput { use_prototypes, struct_item, impl_item })
    }
}

#[proc_macro]
#[proc_macro_error]
pub fn object(input: TokenStream) -> TokenStream {
    let ObjectInput { use_prototypes, struct_item, impl_item } = syn::parse_macro_input!(input as ObjectInput);

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
    let mut proxy_methods = Vec::new();

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
            
            // Store proxy method info
            proxy_methods.push((
                method_name.clone(),
                wrapper_fn_name.to_string(),
                arg_names.clone(),
                arg_types.clone(),
                method_output.clone(),
                method.sig.inputs.clone(),
            ));
        }
    }

    // Write signatures to file
    write_signatures(&struct_name.to_string(), &method_signatures);
    
    // Write proxy module
    write_proxy_module(&struct_name.to_string(), &proxy_methods, &rustc_commit);

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

    // Generate extension trait for prototypes if any
    let prototype_trait = if let Some(ref protos) = use_prototypes {
        generate_prototype_extensions(&protos, &rustc_commit)
    } else {
        quote! {}
    };

    TokenStream::from(quote! {
        #output
        #prototype_trait
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

fn write_proxy_module(struct_name: &str, proxy_methods: &[(syn::Ident, String, Vec<&syn::Ident>, Vec<&Type>, ReturnType, syn::punctuated::Punctuated<FnArg, syn::Token![,]>)], _rustc_commit: &str) {
    use std::fmt::Write;
    
    let mut proxy_content = String::new();
    
    // Header
    writeln!(proxy_content, "// Auto-generated proxy module for {}", struct_name).ok();
    writeln!(proxy_content, "// Include this module in objects that need to call {} methods", struct_name).ok();
    writeln!(proxy_content, "").ok();
    writeln!(proxy_content, "use hotline::ObjectRef;").ok();
    writeln!(proxy_content, "").ok();
    writeln!(proxy_content, "/// Type marker for {}", struct_name).ok();
    writeln!(proxy_content, "pub struct {};", struct_name).ok();
    writeln!(proxy_content, "").ok();
    writeln!(proxy_content, "/// Extension methods for ObjectRef<{}>", struct_name).ok();
    writeln!(proxy_content, "pub trait {}Proxy {{", struct_name).ok();
    
    // Trait method signatures
    for (method_name, _, _arg_names, _arg_types, output, inputs) in proxy_methods {
        write!(proxy_content, "    fn {}(", method_name).ok();
        
        // Write parameters
        let mut first = true;
        for arg in inputs {
            if !first {
                write!(proxy_content, ", ").ok();
            }
            first = false;
            match arg {
                FnArg::Receiver(receiver) => {
                    if receiver.mutability.is_some() {
                        write!(proxy_content, "&mut self").ok();
                    } else {
                        write!(proxy_content, "&self").ok();
                    }
                }
                FnArg::Typed(pat_type) => {
                    write!(proxy_content, "{}", quote! { #pat_type }).ok();
                }
            }
        }
        
        // Write return type
        match output {
            ReturnType::Default => writeln!(proxy_content, ");").ok(),
            ReturnType::Type(_, ty) => writeln!(proxy_content, ") -> {};", quote! { #ty }).ok(),
        };
    }
    
    writeln!(proxy_content, "}}").ok();
    writeln!(proxy_content, "").ok();
    
    // Implementation
    writeln!(proxy_content, "impl {}Proxy for ObjectRef<{}> {{", struct_name, struct_name).ok();
    
    for (method_name, symbol_name, arg_names, arg_types, output, inputs) in proxy_methods {
        // Method signature
        write!(proxy_content, "    fn {}(", method_name).ok();
        
        let mut param_names = Vec::new();
        let mut first = true;
        for arg in inputs {
            if !first {
                write!(proxy_content, ", ").ok();
            }
            first = false;
            match arg {
                FnArg::Receiver(receiver) => {
                    if receiver.mutability.is_some() {
                        write!(proxy_content, "&mut self").ok();
                    } else {
                        write!(proxy_content, "&self").ok();
                    }
                }
                FnArg::Typed(pat_type) => {
                    if let Pat::Ident(pat_ident) = &*pat_type.pat {
                        param_names.push(pat_ident.ident.to_string());
                    }
                    write!(proxy_content, "{}", quote! { #pat_type }).ok();
                }
            }
        }
        
        // Return type
        let return_type_str = match output {
            ReturnType::Default => "".to_string(),
            ReturnType::Type(_, ty) => format!(" -> {}", quote! { #ty }),
        };
        writeln!(proxy_content, "){} {{", return_type_str).ok();
        
        // Method body
        writeln!(proxy_content, "        crate::with_library_registry(|registry| {{").ok();
        
        let is_mut = inputs.first().map_or(false, |arg| {
            if let FnArg::Receiver(receiver) = arg {
                receiver.mutability.is_some()
            } else {
                false
            }
        });
        
        if is_mut {
            writeln!(proxy_content, "            if let Ok(mut guard) = self.inner().lock() {{").ok();
            writeln!(proxy_content, "                let obj_any = guard.as_any_mut();").ok();
        } else {
            writeln!(proxy_content, "            if let Ok(guard) = self.inner().lock() {{").ok();
            writeln!(proxy_content, "                let obj_any = guard.as_any();").ok();
        }
        
        // Function type
        write!(proxy_content, "                type FnType = unsafe extern \"Rust\" fn(&{} dyn std::any::Any", if is_mut { "mut" } else { "" }).ok();
        for ty in arg_types {
            write!(proxy_content, ", {}", quote! { #ty }).ok();
        }
        let return_type = match output {
            ReturnType::Default => quote! { () },
            ReturnType::Type(_, ty) => quote! { #ty },
        };
        writeln!(proxy_content, ") -> {};", return_type).ok();
        
        // Call with symbol
        writeln!(proxy_content, "                registry.with_symbol::<FnType, _, _>(").ok();
        writeln!(proxy_content, "                    \"lib{}\",", struct_name).ok();
        writeln!(proxy_content, "                    \"{}\",", symbol_name).ok();
        write!(proxy_content, "                    |fn_ptr| unsafe {{ (**fn_ptr)(obj_any").ok();
        for name in arg_names.iter() {
            write!(proxy_content, ", {}", name).ok();
        }
        writeln!(proxy_content, ") }}").ok();
        writeln!(proxy_content, "                ).unwrap_or_else(|e| panic!(\"Failed to call {}: {{}}\", e))", method_name).ok();
        writeln!(proxy_content, "            }} else {{").ok();
        writeln!(proxy_content, "                panic!(\"Failed to lock object for method {}\")", method_name).ok();
        writeln!(proxy_content, "            }}").ok();
        writeln!(proxy_content, "        }}).unwrap_or_else(|| panic!(\"No library registry available for method {}\"))", method_name).ok();
        writeln!(proxy_content, "    }}").ok();
        writeln!(proxy_content, "").ok();
    }
    
    writeln!(proxy_content, "}}").ok();
    
    // Write to file in src directory
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let src_dir = Path::new(&manifest_dir).join("src");
        let proxy_file = src_dir.join("proxy.rs");
        fs::write(&proxy_file, proxy_content).ok();
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

fn generate_prototype_extensions(prototypes: &UsePrototypes, rustc_commit: &str) -> proc_macro2::TokenStream {
    let mut trait_methods = Vec::new();
    let mut impl_methods = Vec::new();
    
    // Read and parse each prototype
    for (type_name, method_name) in &prototypes.prototypes {
        // Find and read the .sig file
        let sig_path = find_sig_file(type_name);
        if let Ok(content) = fs::read_to_string(&sig_path) {
            // Find the method signature
            for line in content.lines() {
                if line.starts_with(&format!("{}:", method_name)) {
                    if let Some(parsed) = parse_signature_line(line) {
                        let (method, params, return_type) = parsed;
                        
                        // Generate trait method
                        let method_ident = quote::format_ident!("{}", method);
                        let param_types: Vec<proc_macro2::TokenStream> = params.iter().map(|p| {
                            let ty = parse_type_string(p);
                            quote! { #ty }
                        }).collect();
                        
                        let return_ty = parse_type_string(&return_type);
                        
                        // Generate parameter names
                        let param_names: Vec<proc_macro2::TokenStream> = params.iter().enumerate().map(|(i, _)| {
                            let name = quote::format_ident!("arg{}", i);
                            quote! { #name }
                        }).collect();
                        
                        // Trait method
                        trait_methods.push(quote! {
                            fn #method_ident(&mut self #(, #param_names: #param_types)*) -> #return_ty;
                        });
                        
                        // Implementation method
                        let fn_type = quote! {
                            unsafe extern "Rust" fn(&mut dyn std::any::Any #(, #param_types)*) -> #return_ty
                        };
                        
                        // Build the exact symbol name format
                        let params_part = if params.is_empty() {
                            String::new()
                        } else {
                            let param_parts: Vec<String> = params.iter().enumerate().map(|(i, p)| {
                                format!("__arg{}__{}", i, p)
                            }).collect();
                            param_parts.join("")
                        };
                        
                        // Convert return type to symbol format
                        let return_part = if return_type.starts_with("(") && return_type.ends_with(")") {
                            let inner = &return_type[1..return_type.len()-1];
                            if inner.is_empty() {
                                "unit".to_string()
                            } else {
                                format!("tuple_{}", inner.replace(",", "_comma_"))
                            }
                        } else if return_type == "()" {
                            "unit".to_string()
                        } else {
                            return_type.clone()
                        };
                        
                        impl_methods.push(quote! {
                            fn #method_ident(&mut self #(, #param_names: #param_types)*) -> #return_ty {
                                crate::with_library_registry(|registry| {
                                    if let Ok(mut guard) = self.lock() {
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
                                            |fn_ptr| unsafe { (**fn_ptr)(obj_any #(, #param_names)*) }
                                        ).unwrap_or_else(|e| panic!("Target object {} doesn't have {} method: {}", type_name, stringify!(#method_ident), e))
                                    } else {
                                        panic!("Failed to lock object for method {}", stringify!(#method_ident))
                                    }
                                }).unwrap_or_else(|| panic!("No library registry available for method {}", stringify!(#method_ident)))
                            }
                        });
                        
                        break;
                    }
                }
            }
        }
    }
    
    if trait_methods.is_empty() {
        return quote! {};
    }
    
    quote! {
        trait ObjectHandleExt {
            #(#trait_methods)*
        }
        
        impl ObjectHandleExt for ::hotline::ObjectHandle {
            #(#impl_methods)*
        }
    }
}

fn find_sig_file(type_name: &str) -> std::path::PathBuf {
    // Get the workspace root
    let mut current_dir = std::env::current_dir().unwrap();
    loop {
        let sig_path = current_dir.join("signatures").join(format!("{}.sig", type_name));
        if sig_path.exists() {
            return sig_path;
        }
        if !current_dir.pop() {
            panic!("Could not find {}.sig file", type_name);
        }
    }
}

fn parse_signature_line(line: &str) -> Option<(String, Vec<String>, String)> {
    let parts: Vec<&str> = line.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    
    let method_name = parts[0].to_string();
    let params = if parts[1].is_empty() {
        Vec::new()
    } else {
        parts[1].split(',').map(|s| s.trim().to_string()).collect()
    };
    let return_type = parts[2].to_string();
    
    Some((method_name, params, return_type))
}

fn parse_type_string(type_str: &str) -> proc_macro2::TokenStream {
    match type_str.trim() {
        "()" => quote! { () },
        "bool" => quote! { bool },
        "i64" => quote! { i64 },
        "f64" => quote! { f64 },
        "u8" => quote! { u8 },
        "ObjectHandle" => quote! { ::hotline::ObjectHandle },
        "mut_ref_slice_u8" => quote! { &mut [u8] },
        s if s.starts_with("(") && s.ends_with(")") => {
            // Parse tuple types
            let inner = &s[1..s.len()-1];
            let types: Vec<proc_macro2::TokenStream> = inner
                .split(',')
                .map(|t| parse_type_string(t.trim()))
                .collect();
            quote! { (#(#types),*) }
        }
        _ => panic!("Unknown type in signature: {}", type_str),
    }
}
