use quote::quote;
use syn::{Ident, Type};

pub struct FfiWrapper {
    struct_name: Ident,
    wrapper_name: Ident,
    params: Vec<(Ident, proc_macro2::TokenStream)>,
    return_type: Option<proc_macro2::TokenStream>,
    body: proc_macro2::TokenStream,
    is_mut_receiver: bool,
    is_async: bool,
}

impl FfiWrapper {
    pub fn new(struct_name: Ident, wrapper_name: Ident) -> Self {
        Self {
            struct_name,
            wrapper_name,
            params: vec![],
            return_type: None,
            body: quote! {},
            is_mut_receiver: true,
            is_async: false,
        }
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

    pub fn with_mut_receiver(mut self, is_mut: bool) -> Self {
        self.is_mut_receiver = is_mut;
        self
    }

    pub fn with_async(mut self, is_async: bool) -> Self {
        self.is_async = is_async;
        self
    }

    pub fn build(self) -> proc_macro2::TokenStream {
        let Self { struct_name, wrapper_name, params, return_type, body, is_mut_receiver, is_async } = self;

        let param_list = params.iter().map(|(name, ty)| quote! { #name: #ty });
        let return_spec = return_type.map(|ty| quote! { -> #ty }).unwrap_or_default();

        let doc_comment = if is_async {
            quote! {
                #[doc = "FFI wrapper for async method - uses hotline runtime"]
            }
        } else {
            quote! {}
        };

        if is_mut_receiver {
            quote! {
                #doc_comment
                #[unsafe(no_mangle)]
                #[allow(non_snake_case)]
                pub extern "Rust" fn #wrapper_name(
                    obj: &mut dyn ::std::any::Any
                    #(, #param_list)*
                ) #return_spec {
                    // Skip TypeId check for hot reload compatibility
                    // The symbol name contains full type info, so if we got here, types are compatible
                    let instance = unsafe {
                        &mut *(obj as *mut dyn ::std::any::Any as *mut #struct_name)
                    };
                    #body
                }
            }
        } else {
            quote! {
                #doc_comment
                #[unsafe(no_mangle)]
                #[allow(non_snake_case)]
                pub extern "Rust" fn #wrapper_name(
                    obj: &dyn ::std::any::Any
                    #(, #param_list)*
                ) #return_spec {
                    // Skip TypeId check for hot reload compatibility
                    // The symbol name contains full type info, so if we got here, types are compatible
                    let instance = unsafe {
                        &*(obj as *const dyn ::std::any::Any as *const #struct_name)
                    };
                    #body
                }
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
    is_mut_receiver: bool,
) -> proc_macro2::TokenStream {
    use crate::constants::{ERR_LOCK_FAILED, ERR_METHOD_NOT_FOUND, ERR_NO_REGISTRY};

    if is_mut_receiver {
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
    } else {
        quote! {
            {
                if let Ok(guard) = #receiver.lock() {
                    let obj = &**guard;
                    let type_name = obj.type_name().to_string();
                    let __lib_name = format!("lib{}", type_name);

                    // Get registry from the object using the trait method
                    let registry = obj.get_registry()
                        .unwrap_or_else(|| panic!(concat!(#ERR_NO_REGISTRY, " {}"), #method_name));

                    let obj_any = obj.as_any();

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
}
