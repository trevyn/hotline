use quote::quote;
use std::collections::HashSet;
use std::fs;
use syn::{Item, ItemEnum, ItemStruct};

use crate::codegen::type_hash::{generate_type_hash, generate_type_hash_const};

pub fn generate_custom_type_proxies_for_types(custom_types: &HashSet<String>) -> proc_macro2::TokenStream {
    let mut all_proxies = vec![];
    let mut additional_types = HashSet::new();

    // For each custom type referenced, try to find its definition
    for type_name in custom_types {
        // First check if it's defined in an object library
        if let Some((proxy, deps)) = find_and_generate_proxy_with_deps(type_name) {
            all_proxies.push(proxy);
            additional_types.extend(deps);
        }
    }

    // Also generate proxies for dependent types
    for type_name in &additional_types {
        if !custom_types.contains(type_name) {
            if let Some((proxy, _)) = find_and_generate_proxy_with_deps(type_name) {
                all_proxies.push(proxy);
            }
        }
    }

    quote! { #(#all_proxies)* }
}

fn find_and_generate_proxy_with_deps(
    type_name: &str,
) -> Option<(proc_macro2::TokenStream, std::collections::HashSet<String>)> {
    // Try to find in each object library
    let workspace = find_workspace_root()?;
    let objects_dir = workspace.join("objects");

    // Search all object libraries
    for entry in fs::read_dir(&objects_dir).ok()? {
        let entry = entry.ok()?;
        let lib_file = entry.path().join("src").join("lib.rs");

        if lib_file.exists() {
            if let Ok(content) = fs::read_to_string(&lib_file) {
                if let Ok(file) = syn::parse_file(&content) {
                    // Look for the type definition
                    for item in &file.items {
                        match item {
                            Item::Struct(item_struct) if item_struct.ident == type_name => {
                                return Some((generate_struct_proxy(item_struct), std::collections::HashSet::new()));
                            }
                            Item::Enum(item_enum) if item_enum.ident == type_name => {
                                return Some((generate_enum_proxy(item_enum), std::collections::HashSet::new()));
                            }
                            Item::Macro(item_macro)
                                if item_macro.mac.path.is_ident("object")
                                    || (item_macro.mac.path.segments.len() == 2
                                        && item_macro.mac.path.segments[0].ident == "hotline"
                                        && item_macro.mac.path.segments[1].ident == "object") =>
                            {
                                // Parse types from inside the object! macro
                                if let Ok(parsed) =
                                    syn::parse2::<crate::parser::ObjectInput>(item_macro.mac.tokens.clone())
                                {
                                    // Check if the main struct matches
                                    if parsed.struct_item.ident == type_name {
                                        // This is an object type, not a custom type - we shouldn't generate a proxy for it
                                        // as it already has its own object wrapper
                                        return None;
                                    }

                                    // Collect all type names from this object
                                    let mut deps = std::collections::HashSet::new();
                                    for td in &parsed.type_defs {
                                        match td {
                                            Item::Struct(s) => {
                                                deps.insert(s.ident.to_string());
                                            }
                                            Item::Enum(e) => {
                                                deps.insert(e.ident.to_string());
                                            }
                                            _ => {}
                                        }
                                    }

                                    // Check additional types defined in the object
                                    for type_def in &parsed.type_defs {
                                        match type_def {
                                            Item::Struct(s) if s.ident == type_name => {
                                                return Some((generate_struct_proxy(s), deps));
                                            }
                                            Item::Enum(e) if e.ident == type_name => {
                                                return Some((generate_enum_proxy(e), deps));
                                            }
                                            _ => continue,
                                        }
                                    }
                                }
                            }
                            _ => continue,
                        }
                    }
                }
            }
        }
    }

    None
}

fn generate_struct_proxy(original: &ItemStruct) -> proc_macro2::TokenStream {
    let hash = generate_type_hash(original);
    let hash_const = generate_type_hash_const(&original.ident, hash);

    // Clone the struct but make sure it's public
    let mut proxy = original.clone();
    proxy.vis = syn::parse_quote!(pub);

    quote! {
        #hash_const
        #proxy
    }
}

fn generate_enum_proxy(original: &ItemEnum) -> proc_macro2::TokenStream {
    // Clone the enum but make sure it's public
    let mut proxy = original.clone();
    proxy.vis = syn::parse_quote!(pub);

    quote! {
        #proxy
    }
}

fn find_workspace_root() -> Option<std::path::PathBuf> {
    let find_workspace = |start: std::path::PathBuf| -> Option<std::path::PathBuf> {
        start.ancestors().find(|d| d.join("Cargo.toml").exists() && d.join("objects").exists()).map(|p| p.to_path_buf())
    };

    std::env::current_dir()
        .ok()
        .and_then(find_workspace)
        .or_else(|| std::env::var("OUT_DIR").ok().and_then(|s| find_workspace(s.into())))
}
