use proc_macro::TokenStream;
use proc_macro_error2::proc_macro_error;
use quote::{ToTokens, quote};
use std::collections::HashSet;
use std::fs;
use std::process::Command;
use syn::Type;

mod codegen;
mod constants;
mod discovery;
mod parser;
mod utils;

use codegen::core::generate_core_functions;
use codegen::custom_types::generate_custom_type_proxies_for_types;
use codegen::fields::{generate_default_impl, generate_field_accessors, generate_setter_builder_methods};
use codegen::methods::generate_method_wrappers;
use codegen::process_struct_attributes;
use codegen::serde_impl::{generate_migrate_children_impl, generate_state_serialization};
use codegen::wrapper::generate_typed_wrappers;
use discovery::{extract_object_methods, find_referenced_custom_types, find_referenced_object_types};
use parser::ObjectInput;
use syn::Fields;

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

fn add_registry_field(struct_item: &syn::ItemStruct) -> proc_macro2::TokenStream {
    let mut modified = struct_item.clone();

    // Add the registry field to the struct using RegistryPtr to avoid any TLS
    if let Fields::Named(ref mut fields) = modified.fields {
        let registry_field: syn::Field = syn::parse_quote! {
            #[doc(hidden)]
            #[serde(skip)]
            __hotline_registry: ::hotline::RegistryPtr
        };
        fields.named.push(registry_field);

        // Add object ID field
        let id_field: syn::Field = syn::parse_quote! {
            #[doc(hidden)]
            #[serde(skip)]
            __hotline_object_id: u64
        };
        fields.named.push(id_field);
    }

    // Add Serialize and Deserialize derives if not already present
    let has_serialize = modified.attrs.iter().any(|attr| {
        if attr.path().is_ident("derive") { attr.to_token_stream().to_string().contains("Serialize") } else { false }
    });

    if !has_serialize {
        let derive_attr: syn::Attribute = syn::parse_quote! {
            #[derive(::hotline::serde::Serialize, ::hotline::serde::Deserialize)]
        };
        modified.attrs.push(derive_attr);

        let serde_crate_attr: syn::Attribute = syn::parse_quote! {
            #[serde(crate = "::hotline::serde")]
        };
        modified.attrs.push(serde_crate_attr);
    }

    quote! { #modified }
}

#[proc_macro]
#[proc_macro_error]
pub fn object(input: TokenStream) -> TokenStream {
    let ObjectInput { type_defs, struct_item, impl_blocks } = syn::parse_macro_input!(input as ObjectInput);
    let struct_name = &struct_item.ident;
    let rustc_commit = get_rustc_commit_hash();

    // Get names of types defined in this object to exclude them from proxies (before moving)
    let local_type_names: HashSet<String> = type_defs
        .iter()
        .filter_map(|td| match td {
            syn::Item::Struct(s) => Some(s.ident.to_string()),
            syn::Item::Enum(e) => Some(e.ident.to_string()),
            _ => None,
        })
        .collect();

    // Process type definitions to add Serialize/Deserialize derives
    let processed_type_defs = type_defs
        .into_iter()
        .map(|mut td| {
            match &mut td {
                syn::Item::Struct(s) => {
                    let has_serialize = s.attrs.iter().any(|attr| {
                        if attr.path().is_ident("derive") {
                            attr.to_token_stream().to_string().contains("Serialize")
                        } else {
                            false
                        }
                    });

                    if !has_serialize {
                        let derive_attr: syn::Attribute = syn::parse_quote! {
                            #[derive(::hotline::serde::Serialize, ::hotline::serde::Deserialize)]
                        };
                        s.attrs.push(derive_attr);

                        let serde_crate_attr: syn::Attribute = syn::parse_quote! {
                            #[serde(crate = "::hotline::serde")]
                        };
                        s.attrs.push(serde_crate_attr);
                    }
                }
                syn::Item::Enum(e) => {
                    let has_serialize = e.attrs.iter().any(|attr| {
                        if attr.path().is_ident("derive") {
                            attr.to_token_stream().to_string().contains("Serialize")
                        } else {
                            false
                        }
                    });

                    if !has_serialize {
                        let derive_attr: syn::Attribute = syn::parse_quote! {
                            #[derive(::hotline::serde::Serialize, ::hotline::serde::Deserialize)]
                        };
                        e.attrs.push(derive_attr);

                        let serde_crate_attr: syn::Attribute = syn::parse_quote! {
                            #[serde(crate = "::hotline::serde")]
                        };
                        e.attrs.push(serde_crate_attr);
                    }
                }
                _ => {}
            }
            td
        })
        .collect::<Vec<_>>();

    let processed = process_struct_attributes(&struct_item);

    // Partition impl blocks
    let (main_impl, other_impl_blocks) = {
        let mut main = None;
        let mut others = Vec::new();

        for impl_block in &impl_blocks {
            if impl_block.trait_.is_none()
                && matches!(&*impl_block.self_ty,
                Type::Path(tp) if tp.path.is_ident(struct_name))
            {
                main = Some(impl_block);
            } else {
                others.push(impl_block);
            }
        }

        (main.expect("Expected impl block for struct"), others)
    };

    // Check for Default trait. Use the processed struct to account for removed
    // derives when field defaults are present.
    let has_derive_default = processed
        .modified_struct
        .attrs
        .iter()
        .any(|a| a.path().is_ident("derive") && a.to_token_stream().to_string().contains("Default"));

    let has_impl_default = other_impl_blocks
        .iter()
        .any(|ib| ib.trait_.as_ref().and_then(|(_, p, _)| p.segments.last()).map_or(false, |s| s.ident == "Default"));

    let should_generate_default = !has_impl_default && !processed.field_defaults.is_empty();
    let has_default = has_impl_default || !processed.field_defaults.is_empty() || has_derive_default;

    // Filter impl blocks if generating Default
    let filtered_impl_blocks: Vec<_> = if should_generate_default {
        other_impl_blocks
            .into_iter()
            .filter(|&impl_block| {
                !impl_block
                    .trait_
                    .as_ref()
                    .map(|(_, path, _)| path.segments.last().map(|seg| seg.ident == "Default").unwrap_or(false))
                    .unwrap_or(false)
            })
            .collect()
    } else {
        other_impl_blocks
    };

    // Generate all components
    let field_accessors = generate_field_accessors(struct_name, &processed, &rustc_commit);
    let method_wrappers = generate_method_wrappers(struct_name, main_impl, &processed, &rustc_commit);
    let core_functions = generate_core_functions(struct_name, &rustc_commit, has_default);
    let setter_builder_impl = generate_setter_builder_methods(struct_name, &processed);
    let default_impl =
        should_generate_default.then(|| generate_default_impl(struct_name, &processed)).unwrap_or_default();
    let state_serialization = generate_state_serialization(struct_name, &processed);
    let migrate_children_impl = generate_migrate_children_impl(struct_name, &processed);
    let referenced_objects = find_referenced_object_types(&struct_item, &impl_blocks);

    // Collect all objects and custom types transitively
    let mut all_objects = referenced_objects.clone();
    let mut objects_to_check = referenced_objects.clone();
    let mut all_custom_types = HashSet::new();

    let mut iteration = 0;
    while !objects_to_check.is_empty() && iteration < 5 {
        iteration += 1;

        let mut new_objects = HashSet::new();

        // Check each object for its dependencies
        for object_name in &objects_to_check {
            let lib_path = crate::discovery::find_object_lib_file(object_name);
            if lib_path.exists() {
                if let Ok(content) = fs::read_to_string(&lib_path) {
                    if let Ok(file) = syn::parse_file(&content) {
                        // Collect custom types from this object
                        let mut file_types = HashSet::new();
                        crate::codegen::wrapper::collect_custom_types_from_file(&file, &mut file_types);
                        all_custom_types.extend(file_types);

                        // Get methods and collect types from method signatures
                        if let Some(methods) = extract_object_methods(&file, object_name) {
                            for (_, _, param_types, return_type, _) in &methods {
                                let mut method_types = HashSet::new();
                                crate::codegen::wrapper::collect_custom_types_from_type(return_type, &mut method_types);
                                for param_type in param_types {
                                    crate::codegen::wrapper::collect_custom_types_from_type(
                                        param_type,
                                        &mut method_types,
                                    );
                                }

                                // Separate into objects and custom types
                                for type_name in method_types {
                                    let type_lib_path = crate::discovery::find_object_lib_file(&type_name);
                                    if type_lib_path.exists() && !all_objects.contains(&type_name) {
                                        new_objects.insert(type_name);
                                    } else if !type_lib_path.exists() {
                                        all_custom_types.insert(type_name);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Add new objects for next iteration
        all_objects.extend(new_objects.clone());
        objects_to_check = new_objects;
    }

    // Generate wrappers only for directly referenced objects in this object
    let (additional_wrappers, wrapper_custom_types) = generate_typed_wrappers(&referenced_objects, &rustc_commit);

    // Separate wrapper_custom_types into objects and actual custom types
    let mut transitive_objects = HashSet::new();
    let mut transitive_custom_types = HashSet::new();
    for type_name in wrapper_custom_types {
        // Never generate a wrapper for ourselves
        if type_name == struct_name.to_string() {
            continue;
        }

        let type_lib_path = crate::discovery::find_object_lib_file(&type_name);
        if type_lib_path.exists() {
            // It's an object type - only add if not already directly referenced
            if !referenced_objects.contains(&type_name) {
                transitive_objects.insert(type_name);
            }
        } else {
            // It's a custom type (enum, struct, etc.)
            transitive_custom_types.insert(type_name);
        }
    }

    // Generate wrappers for transitively discovered objects
    let (transitive_wrappers, _) = generate_typed_wrappers(&transitive_objects, &rustc_commit);

    // Get custom types referenced in THIS object only (not transitively discovered)
    let mut local_custom_types = find_referenced_custom_types(&struct_item, &impl_blocks);
    // Also include custom types discovered from imported object wrappers
    local_custom_types.extend(transitive_custom_types);

    // Collect custom types from imported objects that we need proxies for
    let mut custom_types_from_objects = HashSet::new();
    // Include both directly referenced and transitively discovered objects
    let mut all_referenced_objects = referenced_objects.clone();
    all_referenced_objects.extend(transitive_objects.clone());
    for obj_name in &all_referenced_objects {
        let lib_path = crate::discovery::find_object_lib_file(obj_name);
        if lib_path.exists() {
            if let Ok(content) = fs::read_to_string(&lib_path) {
                if let Ok(file) = syn::parse_file(&content) {
                    // Look for custom types defined in object! macro
                    for item in &file.items {
                        if let syn::Item::Macro(item_macro) = item {
                            if item_macro.mac.path.is_ident("object")
                                || (item_macro.mac.path.segments.len() == 2
                                    && item_macro.mac.path.segments[0].ident == "hotline"
                                    && item_macro.mac.path.segments[1].ident == "object")
                            {
                                if let Ok(parsed) =
                                    syn::parse2::<crate::parser::ObjectInput>(item_macro.mac.tokens.clone())
                                {
                                    // Skip the main object type itself
                                    let _main_type = parsed.struct_item.ident.to_string();

                                    // Collect other types defined in the object
                                    for type_def in &parsed.type_defs {
                                        match type_def {
                                            syn::Item::Struct(s) => {
                                                custom_types_from_objects.insert(s.ident.to_string());
                                            }
                                            syn::Item::Enum(e) => {
                                                custom_types_from_objects.insert(e.ident.to_string());
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Add custom types from imported objects to our local custom types
    local_custom_types.extend(custom_types_from_objects);

    // Only generate proxies for:
    // 1. Custom types used in THIS object (including from imported objects)
    // 2. Not defined locally in this object
    // 3. Not the imported object types themselves (they have their own wrappers)
    let external_custom_types: HashSet<String> = local_custom_types
        .into_iter()
        .filter(|t| !local_type_names.contains(t))
        .filter(|t| !referenced_objects.contains(t)) // Not an imported object itself
        .collect();

    let custom_type_proxies = generate_custom_type_proxies_for_types(&external_custom_types);

    let modified_struct = &processed.modified_struct;

    // Add registry field to the struct
    let struct_with_registry = add_registry_field(modified_struct);

    let output = quote! {
        use ::hotline::HotlineObject;

        #[allow(dead_code)]
        type Like<T> = T;

        #(#processed_type_defs)*

        #custom_type_proxies

        #struct_with_registry
        #main_impl
        #(#filtered_impl_blocks)*
        #default_impl

        impl ::hotline::HotlineObject for #struct_name {
            fn type_name(&self) -> &'static str { stringify!(#struct_name) }
            fn object_id(&self) -> u64 { self.__hotline_object_id }
            fn as_any(&self) -> &dyn ::std::any::Any { self }
            fn as_any_mut(&mut self) -> &mut dyn ::std::any::Any { self }
            fn set_registry(&mut self, registry: &'static ::hotline::LibraryRegistry) {
                self.__hotline_registry.set(registry);
            }
            fn get_registry(&self) -> Option<&'static ::hotline::LibraryRegistry> {
                self.__hotline_registry.get()
            }
            fn serialize_state(&self) -> Result<Vec<u8>, String> {
                self.__serialize_state_impl()
            }
            fn deserialize_state(&mut self, data: &[u8]) -> Result<(), String> {
                self.__deserialize_state_impl(data)
            }
            fn migrate_children(&mut self, reloaded_libs: &::std::collections::HashSet<String>) -> Result<(), String> {
                self.migrate_children_impl(reloaded_libs)
            }
        }

        #state_serialization
        #migrate_children_impl

        #setter_builder_impl
        #(#field_accessors)*
        #(#method_wrappers)*
        #core_functions
        #additional_wrappers
        #transitive_wrappers
    };

    TokenStream::from(output)
}
