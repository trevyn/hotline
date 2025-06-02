use proc_macro::TokenStream;
use proc_macro_error2::proc_macro_error;
use quote::{quote, ToTokens};
use syn::{parse::{Parse, ParseStream}, ItemStruct, ItemImpl, Fields, Type, PathArguments, GenericArgument, FnArg, ReturnType, ImplItem, Pat, braced};

struct ObjectInput {
    struct_item: ItemStruct,
    impl_item: ItemImpl,
}

impl Parse for ObjectInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        braced!(content in input);
        
        let struct_item: ItemStruct = content.parse()
            .map_err(|e| syn::Error::new(e.span(), "Expected a struct definition"))?;
        let impl_item: ItemImpl = content.parse()
            .map_err(|e| syn::Error::new(e.span(), "Expected an impl block after the struct"))?;
        
        Ok(ObjectInput {
            struct_item,
            impl_item,
        })
    }
}

#[proc_macro]
#[proc_macro_error]
pub fn object(input: TokenStream) -> TokenStream {
    let ObjectInput { struct_item, impl_item } = syn::parse_macro_input!(input as ObjectInput);
    
    let struct_name = &struct_item.ident;
    let struct_attrs = &struct_item.attrs;
    let struct_fields = &struct_item.fields;
    
    // Get RUSTC_COMMIT_HASH
    let rustc_commit = std::env::var("RUSTC_COMMIT_HASH")
        .expect("RUSTC_COMMIT_HASH environment variable not set");
    
    // Generate field accessors
    let mut field_accessors = Vec::new();
    
    if let Fields::Named(fields) = struct_fields {
        for field in &fields.named {
            let field_name = field.ident.as_ref().unwrap();
            let field_type = &field.ty;
            
            // Only generate accessors for simple types
            if !is_generic_type(field_type) {
                let type_str = type_to_string(field_type);
                
                // Getter
                let getter_fn_name = quote::format_ident!(
                    "{}__get_{}____obj_ref_dyn_Any__to__{}__{}",
                    struct_name, field_name, type_str, rustc_commit
                );
                
                field_accessors.push(quote! {
                    #[unsafe(no_mangle)]
                    #[allow(non_snake_case)]
                    pub extern "Rust" fn #getter_fn_name(obj: &dyn ::std::any::Any) -> #field_type {
                        let instance = obj.downcast_ref::<#struct_name>()
                            .expect(concat!("Type mismatch: expected ", stringify!(#struct_name)));
                        instance.#field_name.clone()
                    }
                });
                
                // Setter
                let setter_fn_name = quote::format_ident!(
                    "{}__set_{}____obj_mut_dyn_Any__{}_{}__to__unit__{}",
                    struct_name, field_name, field_name, type_str, rustc_commit
                );
                
                field_accessors.push(quote! {
                    #[unsafe(no_mangle)]
                    #[allow(non_snake_case)]
                    pub extern "Rust" fn #setter_fn_name(obj: &mut dyn ::std::any::Any, value: #field_type) {
                        let instance = obj.downcast_mut::<#struct_name>()
                            .expect(concat!("Type mismatch: expected ", stringify!(#struct_name)));
                        instance.#field_name = value;
                    }
                });
            }
        }
    }
    
    // Generate constructor if Default is derived
    let has_default = struct_attrs.iter().any(|attr| {
        attr.path().is_ident("derive") && 
        attr.to_token_stream().to_string().contains("Default")
    });
    
    let constructor = if has_default {
        let ctor_fn_name = quote::format_ident!(
            "{}__new____to__Box_lt_dyn_Any_gt__{}",
            struct_name, rustc_commit
        );
        quote! {
            #[unsafe(no_mangle)]
            #[allow(non_snake_case)]
            pub extern "Rust" fn #ctor_fn_name() -> Box<dyn ::std::any::Any> {
                Box::new(<#struct_name as Default>::default())
            }
        }
    } else {
        quote! {}
    };
    
    // Generate method wrappers
    let mut method_wrappers = Vec::new();
    
    for item in &impl_item.items {
        if let ImplItem::Fn(method) = item {
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
                    let instance = obj.downcast_mut::<#struct_name>()
                        .expect(concat!("Type mismatch: expected ", stringify!(#struct_name)));
                    instance.#method_name(#(#arg_names),*)
                }
            };
            
            method_wrappers.push(wrapper);
        }
    }
    
    // Generate output
    let output = quote! {
        #struct_item
        
        #impl_item
        
        #constructor
        
        #(#field_accessors)*
        
        #(#method_wrappers)*
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
        _ => "unknown".to_string(),
    }
}