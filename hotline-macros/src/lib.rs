use proc_macro::TokenStream;
use proc_macro_error2::proc_macro_error;
use quote::{ToTokens, quote};
use std::process::Command;
use syn::Type;

mod codegen;
mod constants;
mod discovery;
mod parser;
mod utils;

use codegen::core::generate_core_functions;
use codegen::fields::{generate_default_impl, generate_field_accessors, generate_setter_builder_methods};
use codegen::methods::generate_method_wrappers;
use codegen::process_struct_attributes;
use codegen::wrapper::generate_typed_wrappers;
use discovery::find_referenced_object_types;
use parser::ObjectInput;

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

#[proc_macro]
#[proc_macro_error]
pub fn object(input: TokenStream) -> TokenStream {
    let ObjectInput { struct_item, impl_blocks } = syn::parse_macro_input!(input as ObjectInput);
    let struct_name = &struct_item.ident;
    let rustc_commit = get_rustc_commit_hash();

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

    // Check for Default trait
    let has_derive_default = struct_item
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("derive") && attr.to_token_stream().to_string().contains("Default"));

    let has_impl_default = other_impl_blocks.iter().any(|impl_block| {
        impl_block
            .trait_
            .as_ref()
            .map(|(_, path, _)| path.segments.last().map(|seg| seg.ident == "Default").unwrap_or(false))
            .unwrap_or(false)
    });

    let should_generate_default = !has_derive_default && !has_impl_default && !processed.field_defaults.is_empty();
    let has_default = has_derive_default || has_impl_default || !processed.field_defaults.is_empty();

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
    let typed_wrappers =
        generate_typed_wrappers(&find_referenced_object_types(&struct_item, &impl_blocks), &rustc_commit);

    let modified_struct = &processed.modified_struct;
    let output = quote! {
        #[allow(dead_code)]
        type Like<T> = T;

        #modified_struct
        #main_impl
        #(#filtered_impl_blocks)*
        #default_impl

        impl ::hotline::HotlineObject for #struct_name {
            fn type_name(&self) -> &'static str { stringify!(#struct_name) }
            fn as_any(&self) -> &dyn ::std::any::Any { self }
            fn as_any_mut(&mut self) -> &mut dyn ::std::any::Any { self }
        }

        #setter_builder_impl
        #(#field_accessors)*
        #(#method_wrappers)*
        #core_functions
        #typed_wrappers
    };

    TokenStream::from(output)
}
