pub mod core;
pub mod custom_types;
pub mod ffi;
pub mod fields;
pub mod methods;
pub mod serde_impl;
pub mod type_hash;
pub mod wrapper;

use quote::quote;
use std::collections::{HashMap, HashSet};
use syn::{Fields, ItemStruct};

pub struct ProcessedStruct {
    pub modified_struct: ItemStruct,
    pub fields_with_setters: HashSet<String>,
    pub field_defaults: HashMap<String, syn::Expr>,
}

pub fn process_struct_attributes(struct_item: &ItemStruct) -> ProcessedStruct {
    let mut modified_struct = struct_item.clone();
    let mut fields_with_setters = HashSet::new();
    let mut field_defaults = HashMap::new();

    if let Fields::Named(fields) = &mut modified_struct.fields {
        fields.named.iter_mut().for_each(|field| {
            let field_name = field.ident.as_ref().unwrap().to_string();

            field.attrs.retain(|attr| match attr.path() {
                p if p.is_ident("setter") => {
                    fields_with_setters.insert(field_name.clone());
                    false
                }
                p if p.is_ident("default") => {
                    if let Ok(value) = attr.parse_args::<syn::Expr>() {
                        field_defaults.insert(field_name.clone(), value);
                    }
                    false
                }
                _ => true,
            });
        });
    }

    // Remove `Default` from derives if field defaults are specified so that we
    // can generate our own Default implementation.
    if !field_defaults.is_empty() {
        use syn::punctuated::Punctuated;
        use syn::{Meta, Path, Token};

        let mut new_attrs = Vec::with_capacity(modified_struct.attrs.len());
        for mut attr in modified_struct.attrs.into_iter() {
            if attr.path().is_ident("derive") {
                if let Meta::List(meta_list) = &mut attr.meta {
                    let derives: Punctuated<Path, Token![,]> =
                        meta_list.parse_args_with(Punctuated::parse_terminated).unwrap_or_default();
                    let filtered: Punctuated<Path, Token![,]> =
                        derives.into_iter().filter(|p| !p.is_ident("Default")).collect();
                    if filtered.is_empty() {
                        continue; // Drop the entire attribute
                    }
                    meta_list.tokens = quote!(#filtered).into();
                }
            }
            new_attrs.push(attr);
        }
        modified_struct.attrs = new_attrs;
    }

    ProcessedStruct { modified_struct, fields_with_setters, field_defaults }
}
