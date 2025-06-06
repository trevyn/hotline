use quote::{format_ident, quote};
use std::collections::HashSet;
use std::fs;
use syn::{Item, Type};

use crate::codegen::methods::{MethodGenConfig, generate_method_impl};
use crate::discovery::{ReceiverType, extract_object_methods, find_object_lib_file};

pub fn collect_custom_types_from_type(ty: &Type, types: &mut HashSet<String>) {
    match ty {
        Type::Path(type_path) => {
            if let Some(ident) = type_path.path.get_ident() {
                let name = ident.to_string();
                // Look for custom types (uppercase first letter, not common types)
                if !name.starts_with(char::is_lowercase)
                    && name != "String"
                    && name != "Vec"
                    && name != "Option"
                    && name != "Result"
                    && name != "Self"
                {
                    types.insert(name);
                }
            }
        }
        Type::Reference(type_ref) => {
            collect_custom_types_from_type(&type_ref.elem, types);
        }
        Type::Slice(type_slice) => {
            collect_custom_types_from_type(&type_slice.elem, types);
        }
        Type::Array(type_array) => {
            collect_custom_types_from_type(&type_array.elem, types);
        }
        Type::Ptr(type_ptr) => {
            collect_custom_types_from_type(&type_ptr.elem, types);
        }
        Type::Tuple(type_tuple) => {
            for elem in &type_tuple.elems {
                collect_custom_types_from_type(elem, types);
            }
        }
        _ => {}
    }
}

pub fn collect_custom_types_from_file(file: &syn::File, types: &mut HashSet<String>) {
    for item in &file.items {
        match item {
            Item::Macro(item_macro)
                if item_macro.mac.path.is_ident("object")
                    || (item_macro.mac.path.segments.len() == 2
                        && item_macro.mac.path.segments[0].ident == "hotline"
                        && item_macro.mac.path.segments[1].ident == "object") =>
            {
                // Parse the object! macro to find custom types
                if let Ok(parsed) = syn::parse2::<crate::parser::ObjectInput>(item_macro.mac.tokens.clone()) {
                    // Collect types defined in the object
                    for type_def in &parsed.type_defs {
                        match type_def {
                            Item::Struct(s) => {
                                types.insert(s.ident.to_string());
                            }
                            Item::Enum(e) => {
                                types.insert(e.ident.to_string());
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn generate_typed_wrappers(
    types: &HashSet<String>,
    rustc_commit: &str,
) -> (proc_macro2::TokenStream, HashSet<String>) {
    let mut all_types = HashSet::new();

    let wrappers: Vec<_> = types
        .iter()
        .filter_map(|type_name| {
            let lib_path = find_object_lib_file(type_name);

            // Skip if the lib file doesn't exist (might be an external type)
            if !lib_path.exists() {
                return None;
            }

            fs::read_to_string(&lib_path)
                .ok()
                .and_then(|content| syn::parse_file(&content).ok())
                .and_then(|file| {
                    // Also look for custom types defined in this object file
                    collect_custom_types_from_file(&file, &mut all_types);

                    extract_object_methods(&file, type_name)
                })
                .map(|methods| {
                    // Collect types from method signatures
                    for (_, _, param_types, return_type, _) in &methods {
                        let mut method_types = HashSet::new();
                        collect_custom_types_from_type(return_type, &mut method_types);
                        for param_type in param_types {
                            collect_custom_types_from_type(param_type, &mut method_types);
                        }
                        all_types.extend(method_types);
                    }
                    generate_typed_wrapper(type_name, &methods, rustc_commit)
                })
        })
        .collect();

    (quote! { #(#wrappers)* }, all_types)
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
                    match registry.call_constructor(concat!("lib", #type_name), #type_name, ::hotline::RUSTC_COMMIT) {
                        Ok(obj) => obj,
                        Err(e) => panic!("Failed to construct {}: {}", #type_name, e)
                    }
                }).expect(&format!("Library registry not initialized for {}", #type_name));

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

        impl ::hotline::HotlineObject for #type_ident {
            fn type_name(&self) -> &'static str {
                // Get the actual type name from the wrapped object
                if let Ok(guard) = self.0.lock() {
                    guard.type_name()
                } else {
                    #type_name
                }
            }

            fn as_any(&self) -> &dyn ::std::any::Any {
                self as &dyn ::std::any::Any
            }

            fn as_any_mut(&mut self) -> &mut dyn ::std::any::Any {
                self as &mut dyn ::std::any::Any
            }

            fn set_registry(&mut self, registry: &'static ::hotline::LibraryRegistry) {
                if let Ok(mut guard) = self.0.lock() {
                    guard.set_registry(registry);
                }
            }

            fn get_registry(&self) -> Option<&'static ::hotline::LibraryRegistry> {
                if let Ok(guard) = self.0.lock() {
                    guard.get_registry()
                } else {
                    None
                }
            }
        }
    }
}
