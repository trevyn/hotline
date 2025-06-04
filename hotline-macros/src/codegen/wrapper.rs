use quote::{format_ident, quote};
use std::collections::HashSet;
use std::fs;
use syn::Type;

use crate::codegen::methods::{MethodGenConfig, generate_method_impl};
use crate::discovery::{ReceiverType, extract_object_methods, find_object_lib_file};

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
        .filter_map(|method| {
            let config = MethodGenConfig::from_method(method, type_name);
            let impl_tokens = generate_method_impl(&config, type_name, rustc_commit);
            (!impl_tokens.is_empty()).then_some(impl_tokens)
        })
        .collect();

    quote! {
        #[derive(Clone)]
        pub struct #type_ident(::hotline::ObjectHandle);

        impl #type_ident {
            pub fn new() -> Self {
                // Use thread-local registry to create the object
                let obj = ::hotline::with_library_registry(|registry| {
                    registry.call_constructor(concat!("lib", #type_name), #type_name, ::hotline::RUSTC_COMMIT)
                        .expect(&format!("Failed to construct {}", #type_name))
                }).expect("Library registry not initialized");

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
