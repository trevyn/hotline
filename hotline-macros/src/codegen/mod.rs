pub mod core;
pub mod ffi;
pub mod fields;
pub mod methods;
pub mod wrapper;

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

    ProcessedStruct { modified_struct, fields_with_setters, field_defaults }
}
