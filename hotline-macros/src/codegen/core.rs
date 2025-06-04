use quote::{format_ident, quote};
use syn::Ident;

use crate::constants::ERR_TYPE_MISMATCH;
use crate::utils::symbols::SymbolName;

pub fn generate_core_functions(struct_name: &Ident, rustc_commit: &str, has_default: bool) -> proc_macro2::TokenStream {
    let symbol = SymbolName::new(&struct_name.to_string(), "", rustc_commit);
    let type_name_fn = format_ident!("{}", symbol.build_type_name_getter());

    let constructor = has_default
        .then(|| {
            let ctor_name = format_ident!("{}", symbol.build_constructor());
            quote! {
                #[unsafe(no_mangle)]
                #[allow(non_snake_case)]
                pub extern "Rust" fn #ctor_name() -> Box<dyn ::hotline::HotlineObject> {
                    Box::new(<#struct_name as Default>::default()) as Box<dyn ::hotline::HotlineObject>
                }
            }
        })
        .unwrap_or_default();

    quote! {
        #constructor

        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub extern "Rust" fn #type_name_fn(obj: &dyn ::std::any::Any) -> &'static str {
            obj.downcast_ref::<#struct_name>()
                .map(|_| stringify!(#struct_name))
                .unwrap_or_else(|| panic!(
                    concat!(#ERR_TYPE_MISMATCH, " type_name getter: expected {}, but got {}"),
                    stringify!(#struct_name),
                    ::std::any::type_name_of_val(obj)
                ))
        }

        // No static storage - registry is passed as parameter when needed
    }
}
