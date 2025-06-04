use quote::{format_ident, quote};
use std::collections::HashSet;
use std::fs;
use syn::Type;

use crate::codegen::methods::{MethodGenConfig, generate_method_impl};
use crate::constants::{ERR_CONSTRUCT_FAILED, ERR_REGISTRY_NOT_INIT, LIB_PREFIX, SET_PREFIX, WITH_PREFIX};
use crate::discovery::{ReceiverType, extract_object_methods, find_object_lib_file};
use crate::utils::types::is_object_type;

pub fn generate_typed_wrappers(types: &HashSet<String>, rustc_commit: &str) -> proc_macro2::TokenStream {
    let wrappers: Vec<_> = types
        .iter()
        .filter_map(|type_name| {
            let lib_path = find_object_lib_file(type_name);
            fs::read_to_string(&lib_path)
                .ok()
                .and_then(|content| syn::parse_file(&content).ok())
                .and_then(|file| extract_object_methods(&file, type_name))
                .map(|methods| generate_typed_wrapper(type_name, &methods, rustc_commit))
        })
        .collect();

    quote! { #(#wrappers)* }
}

fn generate_typed_wrapper(
    type_name: &str,
    methods: &[(String, Vec<String>, Vec<Type>, Type, ReceiverType)],
    rustc_commit: &str,
) -> proc_macro2::TokenStream {
    let type_ident = format_ident!("{}", type_name);

    let method_impls: Vec<_> = methods
        .iter()
        .filter_map(|(method_name, param_names, param_types, return_type, receiver_type)| {
            let method_ident = format_ident!("{}", method_name);
            let param_idents: Vec<_> = param_names.iter().map(|name| format_ident!("{}", name)).collect();

            let returns_self = matches!(return_type, Type::Path(tp) if tp.path.is_ident(type_name));
            let is_builder = *receiver_type == ReceiverType::Value && returns_self;
            let setter_name = method_name.strip_prefix(WITH_PREFIX).map(|s| format!("{}{}", SET_PREFIX, s));

            let needs_option_wrap = method_name.starts_with(WITH_PREFIX) && param_types.len() == 1 && {
                let inner = match &param_types[0] {
                    Type::Reference(tr) => &*tr.elem,
                    t => t,
                };
                matches!(inner, Type::Path(tp) if tp.path.get_ident()
                    .map(|i| is_object_type(&i.to_string()))
                    .unwrap_or(false))
            };

            let config = MethodGenConfig {
                method_name: method_name.clone(),
                method_ident,
                param_names: param_names.clone(),
                param_idents,
                param_types: param_types.clone(),
                return_type: return_type.clone(),
                receiver_type: *receiver_type,
                returns_self,
                is_builder,
                needs_option_wrap,
                setter_name,
            };

            let impl_tokens = generate_method_impl(&config, type_name, rustc_commit);
            (!impl_tokens.is_empty()).then_some(impl_tokens)
        })
        .collect();

    quote! {
        #[derive(Clone)]
        pub struct #type_ident(::hotline::ObjectHandle);

        impl #type_ident {
            pub fn new() -> Self {
                let obj = with_library_registry(|registry| {
                    registry.call_constructor(concat!(#LIB_PREFIX, #type_name), #type_name, ::hotline::RUSTC_COMMIT)
                        .expect(&format!(concat!(#ERR_CONSTRUCT_FAILED, " {}"), #type_name))
                }).expect(#ERR_REGISTRY_NOT_INIT);

                Self::from_handle(::std::sync::Arc::new(::std::sync::Mutex::new(obj)))
            }

            pub fn from_handle(handle: ::hotline::ObjectHandle) -> Self {
                Self(handle)
            }

            pub fn handle(&self) -> &::hotline::ObjectHandle {
                &self.0
            }

            #(#method_impls)*
        }

        impl ::std::ops::Deref for #type_ident {
            type Target = ::hotline::ObjectHandle;
            fn deref(&self) -> &Self::Target { &self.0 }
        }

        impl ::std::ops::DerefMut for #type_ident {
            fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
        }
    }
}
