use quote::{format_ident, quote};
use syn::{Fields, Ident, Type};

use crate::codegen::{ProcessedStruct, ffi::FfiWrapper};
use crate::utils::symbols::SymbolName;
use crate::utils::types::{extract_option_type, is_generic_type, is_object_type, type_to_string};

pub fn generate_field_accessors(
    struct_name: &Ident,
    processed: &ProcessedStruct,
    rustc_commit: &str,
) -> Vec<proc_macro2::TokenStream> {
    if let Fields::Named(fields) = &processed.modified_struct.fields {
        fields
            .named
            .iter()
            .filter_map(|field| {
                let field_name = field.ident.as_ref()?;
                let field_type = &field.ty;

                if is_generic_type(field_type) {
                    return None;
                }

                let is_public = matches!(field.vis, syn::Visibility::Public(_));
                let has_setter = processed.fields_with_setters.contains(&field_name.to_string());

                let mut accessors = vec![];
                if is_public {
                    accessors.push(generate_accessor_wrapper(struct_name, field_name, field_type, true, rustc_commit));
                }
                if has_setter {
                    accessors.push(generate_accessor_wrapper(struct_name, field_name, field_type, false, rustc_commit));
                }

                Some(accessors)
            })
            .flatten()
            .collect()
    } else {
        vec![]
    }
}

fn generate_accessor_wrapper(
    struct_name: &Ident,
    field_name: &Ident,
    field_type: &Type,
    is_getter: bool,
    rustc_commit: &str,
) -> proc_macro2::TokenStream {
    let symbol = SymbolName::new(&struct_name.to_string(), &field_name.to_string(), rustc_commit);
    let type_str = type_to_string(field_type);

    if is_getter {
        let wrapper_name = format_ident!("{}", symbol.with_return_type(type_str).build_getter());
        FfiWrapper::new(struct_name.clone(), wrapper_name)
            .returns(Some(field_type))
            .body(quote! { instance.#field_name.clone() })
            .build()
    } else {
        // Use build_method() to match what the proxy expects
        let setter_name = format!("set_{}", field_name);
        let setter_symbol = SymbolName::new(&struct_name.to_string(), &setter_name, rustc_commit)
            .with_params(vec![("value".to_string(), type_str)])
            .with_return_type("unit".to_string());
        let wrapper_name = format_ident!("{}", setter_symbol.build_method());
        let value_ident = format_ident!("value");
        FfiWrapper::new(struct_name.clone(), wrapper_name)
            .param(value_ident, field_type)
            .body(quote! { instance.#field_name = value; })
            .build()
    }
}

pub fn generate_setter_builder_methods(struct_name: &Ident, processed: &ProcessedStruct) -> proc_macro2::TokenStream {
    if let Fields::Named(fields) = &processed.modified_struct.fields {
        let methods: Vec<_> = fields
            .named
            .iter()
            .filter_map(|field| {
                let field_name = field.ident.as_ref()?;
                if !processed.fields_with_setters.contains(&field_name.to_string()) {
                    return None;
                }

                let field_type = &field.ty;
                let setter = format_ident!("set_{}", field_name);
                let builder = format_ident!("with_{}", field_name);

                let (param_type, value_expr) = extract_option_type(field_type)
                    .and_then(|inner| match inner {
                        Type::Path(tp) => tp
                            .path
                            .get_ident()
                            .filter(|i| is_object_type(&i.to_string()))
                            .map(|_| (quote! { &#inner }, quote! { Some(value.clone()) })),
                        _ => None,
                    })
                    .unwrap_or((quote! { #field_type }, quote! { value }));

                Some(quote! {
                    pub fn #setter(&mut self, value: #param_type) {
                        self.#field_name = #value_expr;
                    }

                    pub fn #builder(mut self, value: #param_type) -> Self {
                        self.#field_name = #value_expr;
                        self
                    }
                })
            })
            .collect();

        if !methods.is_empty() {
            quote! { impl #struct_name { #(#methods)* } }
        } else {
            quote! {}
        }
    } else {
        quote! {}
    }
}

pub fn generate_default_impl(struct_name: &Ident, processed: &ProcessedStruct) -> proc_macro2::TokenStream {
    if processed.field_defaults.is_empty() {
        return quote! {};
    }

    if let Fields::Named(fields) = &processed.modified_struct.fields {
        let field_inits: Vec<_> = fields
            .named
            .iter()
            .filter_map(|field| {
                let field_name = field.ident.as_ref()?;

                // Skip the internal registry field
                if field_name == "__hotline_registry" {
                    return None;
                }

                let init = processed
                    .field_defaults
                    .get(&field_name.to_string())
                    .map(|expr| quote! { #field_name: #expr })
                    .unwrap_or_else(|| quote! { #field_name: Default::default() });
                Some(init)
            })
            .collect();

        quote! {
            impl Default for #struct_name {
                fn default() -> Self {
                    Self {
                        #(#field_inits,)*
                        __hotline_registry: ::hotline::RegistryPtr::new()
                    }
                }
            }
        }
    } else {
        quote! {}
    }
}
