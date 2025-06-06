use quote::quote;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use syn::{Fields, ItemStruct, Type};

pub fn generate_type_hash(item: &ItemStruct) -> u64 {
    let mut hasher = DefaultHasher::new();

    // Hash the struct name
    item.ident.to_string().hash(&mut hasher);

    // Hash each field's name and type
    if let Fields::Named(fields) = &item.fields {
        for field in &fields.named {
            if let Some(ident) = &field.ident {
                ident.to_string().hash(&mut hasher);
                hash_type(&field.ty, &mut hasher);
            }
        }
    }

    hasher.finish()
}

fn hash_type(ty: &Type, hasher: &mut DefaultHasher) {
    // Convert type to a normalized string representation
    let type_tokens = quote! { #ty };
    type_tokens.to_string().hash(hasher);
}

pub fn generate_type_hash_const(struct_name: &syn::Ident, hash: u64) -> proc_macro2::TokenStream {
    let const_name = quote::format_ident!("{}_TYPE_HASH", struct_name.to_string().to_uppercase());
    quote! {
        pub const #const_name: u64 = #hash;
    }
}
