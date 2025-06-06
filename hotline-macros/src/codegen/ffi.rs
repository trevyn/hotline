use quote::quote;
use syn::{Ident, Type};

pub struct FfiWrapper {
    struct_name: Ident,
    wrapper_name: Ident,
    params: Vec<(Ident, proc_macro2::TokenStream)>,
    return_type: Option<proc_macro2::TokenStream>,
    body: proc_macro2::TokenStream,
}

impl FfiWrapper {
    pub fn new(struct_name: Ident, wrapper_name: Ident) -> Self {
        Self { struct_name, wrapper_name, params: vec![], return_type: None, body: quote! {} }
    }

    pub fn param(mut self, name: Ident, ty: &Type) -> Self {
        self.params.push((name, quote! { #ty }));
        self
    }

    pub fn params(mut self, params: Vec<(Ident, &Type)>) -> Self {
        for (name, ty) in params {
            self.params.push((name, quote! { #ty }));
        }
        self
    }

    pub fn returns(mut self, ty: Option<&Type>) -> Self {
        self.return_type = ty.map(|t| quote! { #t });
        self
    }

    pub fn body(mut self, body: proc_macro2::TokenStream) -> Self {
        self.body = body;
        self
    }

    pub fn build(self) -> proc_macro2::TokenStream {
        let Self { struct_name, wrapper_name, params, return_type, body } = self;

        let param_list = params.iter().map(|(name, ty)| quote! { #name: #ty });
        let return_spec = return_type.map(|ty| quote! { -> #ty }).unwrap_or_default();
        quote! {
            #[unsafe(no_mangle)]
            #[allow(non_snake_case)]
            pub extern "Rust" fn #wrapper_name(
                obj: &mut dyn ::std::any::Any
                #(, #param_list)*
            ) #return_spec {
                let obj_type_name = ::std::any::type_name_of_val(&*obj);
                let instance = obj.downcast_mut::<#struct_name>()
                    .unwrap_or_else(|| panic!("Type mismatch in {}: expected {}, but got {}",
                        stringify!(#wrapper_name), stringify!(#struct_name), obj_type_name));
                #body
            }
        }
    }
}

pub fn quote_method_call_with_registry(
    receiver: proc_macro2::TokenStream,
    method_name: &str,
    symbol_name: &str,
    fn_type: proc_macro2::TokenStream,
    args: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    use crate::constants::{ERR_LOCK_FAILED, ERR_METHOD_NOT_FOUND, ERR_NO_REGISTRY};

    quote! {
        {
            if let Ok(mut guard) = #receiver.lock() {
                let obj = &mut **guard;
                let type_name = obj.type_name().to_string();
                let __lib_name = format!("lib{}", type_name);

                // Get registry from the object using the trait method
                let registry = obj.get_registry()
                    .unwrap_or_else(|| panic!(concat!(#ERR_NO_REGISTRY, " {}"), #method_name));

                let obj_any = obj.as_any_mut();

                type FnType = #fn_type;
                registry.with_symbol::<FnType, _, _>(
                    &__lib_name,
                    &#symbol_name,
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any #args) }
                ).unwrap_or_else(|e| panic!(#ERR_METHOD_NOT_FOUND, type_name, #method_name, e))
            } else {
                panic!(concat!(#ERR_LOCK_FAILED, " {}"), #method_name)
            }
        }
    }
}
