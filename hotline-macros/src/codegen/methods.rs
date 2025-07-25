use proc_macro_error2::abort;
use quote::{format_ident, quote};
use syn::{Fields, FnArg, Ident, ImplItem, ItemImpl, Pat, ReturnType, Type};

use crate::codegen::{
    ProcessedStruct,
    ffi::{FfiWrapper, quote_method_call_with_registry},
};
use crate::constants::{SET_PREFIX, WITH_PREFIX};
use crate::discovery::ReceiverType;
use crate::utils::symbols::SymbolName;
use crate::utils::types::{
    contains_reference, extract_like_type, extract_option_type, is_object_type, resolve_self_type, type_to_string,
};

pub fn generate_method_wrappers(
    struct_name: &Ident,
    main_impl: &ItemImpl,
    processed: &ProcessedStruct,
    rustc_commit: &str,
) -> Vec<proc_macro2::TokenStream> {
    let mut wrappers = Vec::new();

    // Generate setter wrappers for Option<ObjectType> fields
    if let Fields::Named(fields) = &processed.modified_struct.fields {
        wrappers.extend(fields.named.iter().filter_map(|field| {
            let field_name = field.ident.as_ref()?;
            if !processed.fields_with_setters.contains(&field_name.to_string()) {
                return None;
            }

            extract_option_type(&field.ty)
                .and_then(|inner| match inner {
                    Type::Path(tp) => tp.path.get_ident().map(|i| (inner, i.to_string())),
                    _ => None,
                })
                .filter(|(_, name)| is_object_type(name))
                .map(|(inner_type, _)| {
                    let setter = format_ident!("{}{}", SET_PREFIX, field_name);
                    let type_str = type_to_string(inner_type);
                    let wrapper_name = format_ident!(
                        "{}__{}______obj_mut_dyn_Any____value__ref_{}____to__unit__{}",
                        struct_name,
                        setter,
                        type_str,
                        rustc_commit
                    );
                    FfiWrapper::new(struct_name.clone(), wrapper_name)
                        .param(format_ident!("value"), &syn::parse_quote! { &#inner_type })
                        .body(quote! { instance.#setter(value) })
                        .build()
                })
        }));
    }

    // Generate method wrappers
    wrappers.extend(main_impl.items.iter().filter_map(|item| match item {
        ImplItem::Fn(method) if matches!(method.vis, syn::Visibility::Public(_)) => {
            generate_method_wrapper(struct_name, method, rustc_commit)
        }
        _ => None,
    }));

    wrappers
}

fn generate_method_wrapper(
    struct_name: &Ident,
    method: &syn::ImplItemFn,
    rustc_commit: &str,
) -> Option<proc_macro2::TokenStream> {
    // Check if method is async
    let is_async = method.sig.asyncness.is_some();

    // Check receiver and determine mutability
    let is_mut = match method.sig.inputs.first()? {
        FnArg::Receiver(r) if r.reference.is_some() => r.mutability.is_some(),
        _ => return None,
    };

    let method_name = &method.sig.ident;
    let params: Vec<_> = method
        .sig
        .inputs
        .iter()
        .skip(1)
        .filter_map(|arg| match arg {
            FnArg::Typed(typed) => match &*typed.pat {
                Pat::Ident(pat_ident) => Some((pat_ident.ident.clone(), &*typed.ty)),
                _ => None,
            },
            _ => None,
        })
        .collect();

    let param_specs: Vec<_> = params.iter().map(|(name, ty)| (name.to_string(), type_to_string(ty))).collect();

    let return_type = match &method.sig.output {
        ReturnType::Default => None,
        ReturnType::Type(_, ty) => {
            let resolved_type = resolve_self_type((**ty).clone(), &struct_name.to_string());

            // Check if the return type contains any references
            if contains_reference(&resolved_type) {
                abort!(
                    method.sig.ident.span(),
                    "Methods returning references are not supported in hotline objects";
                    note = "Due to FFI limitations, methods cannot return references (including references inside generic types like Option<&T> or Vec<&T>). Consider alternatives:\n\
                    - Return owned/cloned values\n\
                    - Send messages back to the caller: pass another object and call its methods with the data\n\
                    - Use a visitor pattern: `pub fn visit_data(&self, visitor: &mut DataVisitor)`\n\
                    - Use intent tokens as per Alan Kay's vision: objects send messages, not data"
                );
            }

            Some(resolved_type)
        }
    };

    let symbol = SymbolName::new(&struct_name.to_string(), &method_name.to_string(), rustc_commit)
        .with_params(param_specs)
        .with_return_type(return_type.as_ref().map(type_to_string).unwrap_or_else(|| "unit".to_string()))
        .with_receiver_mutability(is_mut);

    let wrapper_name = format_ident!("{}", symbol.build_method());
    let arg_names: Vec<_> = params.iter().map(|(name, _)| name.clone()).collect();

    Some(
        FfiWrapper::new(struct_name.clone(), wrapper_name)
            .params(params)
            .returns(return_type.as_ref())
            .body(if is_async {
                // For async methods, always block on them in FFI context
                // This is necessary because FFI functions can't be async
                quote! {
                    ::hotline::hotline_runtime().block_on(
                        instance.#method_name(#(#arg_names),*)
                    )
                }
            } else {
                quote! { instance.#method_name(#(#arg_names),*) }
            })
            .with_mut_receiver(is_mut)
            .with_async(is_async)
            .build(),
    )
}

#[derive(Debug)]
pub struct MethodGenConfig {
    pub method_name: String,
    pub method_ident: Ident,
    pub param_names: Vec<String>,
    pub param_idents: Vec<Ident>,
    pub param_types: Vec<Type>,
    pub return_type: Type,
    pub receiver_type: ReceiverType,
    pub returns_self: bool,
    pub is_builder: bool,
    pub needs_option_wrap: bool,
    pub setter_name: Option<String>,
}

impl MethodGenConfig {
    pub fn from_method(method: &(String, Vec<String>, Vec<Type>, Type, ReceiverType), type_name: &str) -> Self {
        let (method_name, param_names, param_types, return_type, receiver_type) = method;
        let returns_self = matches!(return_type, Type::Path(tp) if tp.path.is_ident(type_name));
        let is_builder = *receiver_type == ReceiverType::Value && returns_self;

        let needs_option_wrap = method_name.starts_with(WITH_PREFIX) && param_types.len() == 1 && {
            let inner = match &param_types[0] {
                Type::Reference(tr) => &*tr.elem,
                t => t,
            };
            matches!(inner, Type::Path(tp) if tp.path.get_ident()
                .map(|i| is_object_type(&i.to_string()))
                .unwrap_or(false))
        };

        Self {
            method_name: method_name.clone(),
            method_ident: format_ident!("{}", method_name),
            param_names: param_names.clone(),
            param_idents: param_names.iter().map(|n| format_ident!("{}", n)).collect(),
            param_types: param_types.clone(),
            return_type: return_type.clone(),
            receiver_type: *receiver_type,
            returns_self,
            is_builder,
            needs_option_wrap,
            setter_name: method_name.strip_prefix(WITH_PREFIX).map(|s| format!("{}{}", SET_PREFIX, s)),
        }
    }

    pub fn should_skip(&self) -> bool {
        self.receiver_type == ReceiverType::Value && !self.returns_self
    }

    pub fn is_field_setter(&self) -> bool {
        self.is_builder && self.setter_name.is_some() && self.method_name.starts_with(WITH_PREFIX)
    }
}

pub fn generate_method_impl(config: &MethodGenConfig, type_name: &str, rustc_commit: &str) -> proc_macro2::TokenStream {
    let MethodGenConfig { method_ident, param_idents, param_types, .. } = config;

    if config.should_skip() {
        return quote! {};
    }

    // Simple builder that returns self without doing anything
    if config.is_builder && config.returns_self && !config.needs_option_wrap && !config.is_field_setter() {
        return quote! {
            pub fn #method_ident(self #(, #param_idents: #param_types)*) -> Self { self }
        };
    }

    // Builder methods that need FFI calls
    if config.is_builder && config.returns_self {
        return generate_builder_ffi_method(config, type_name, rustc_commit);
    }

    // Regular methods
    generate_regular_method(config, type_name, rustc_commit)
}

fn generate_builder_ffi_method(
    config: &MethodGenConfig,
    type_name: &str,
    rustc_commit: &str,
) -> proc_macro2::TokenStream {
    let MethodGenConfig { method_ident, param_idents, param_types, .. } = config;

    let symbol_name = if config.needs_option_wrap {
        let inner_type_str = match &param_types[0] {
            Type::Reference(tr) => type_to_string(&*tr.elem),
            t => type_to_string(t),
        };
        format!(
            "{}__{}______obj_mut_dyn_Any____value__ref_{}____to__unit__{}",
            type_name,
            config.setter_name.as_ref().unwrap(),
            inner_type_str,
            rustc_commit
        )
    } else {
        // Builder methods should use build_method() like regular methods
        let param_specs: Vec<_> = config
            .param_names
            .iter()
            .zip(param_types.iter())
            .map(|(name, ty)| (name.clone(), type_to_string(ty)))
            .collect();

        let symbol = SymbolName::new(type_name, &config.setter_name.as_ref().unwrap(), rustc_commit)
            .with_params(param_specs)
            .with_return_type("unit".to_string());

        symbol.build_method()
    };

    quote! {
        pub fn #method_ident(mut self #(, #param_idents: #param_types)*) -> Self {
            if let Ok(mut guard) = self.0.lock() {
                let obj = &mut **guard;
                let type_name = obj.type_name().to_string();
                let __lib_name = format!("lib{}", type_name);

                // Get registry from the object using the trait method
                let registry = obj.get_registry()
                    .unwrap_or_else(|| panic!("Registry not initialized for {}", type_name));

                let obj_any = obj.as_any_mut();

                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any #(, #param_types)*);
                registry.with_symbol::<FnType, _, _>(&__lib_name, &#symbol_name,
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any #(, #param_idents)*) }
                ).unwrap_or_else(|e| panic!("Method not found: {}", e));
            }
            self
        }
    }
}

fn generate_regular_method(config: &MethodGenConfig, type_name: &str, rustc_commit: &str) -> proc_macro2::TokenStream {
    let MethodGenConfig { method_ident, param_idents, param_types, param_names, return_type, receiver_type, .. } =
        config;

    // Check if any parameters are Like<T> where T is an object type
    let mut generic_params = vec![];
    let mut adjusted_param_types = vec![];
    let mut ffi_param_types = vec![];

    for (i, param_type) in param_types.iter().enumerate() {
        let mut handled = false;

        if let Type::Reference(type_ref) = param_type {
            // Check for &[mut] Like<Object>
            if let Some(inner) = extract_like_type(&type_ref.elem) {
                if let Type::Path(tp) = inner {
                    if let Some(ident) = tp.path.get_ident() {
                        if is_object_type(&ident.to_string()) {
                            // This is &mut Like<SomeObject>
                            let generic_name = format_ident!("T{}", i);
                            generic_params.push(quote! { #generic_name: ::hotline::HotlineObject });

                            // Use generic in method signature
                            if type_ref.mutability.is_some() {
                                adjusted_param_types.push(quote! { &mut #generic_name });
                            } else {
                                adjusted_param_types.push(quote! { &#generic_name });
                            }

                            // Use appropriate reference type for FFI
                            if type_ref.mutability.is_some() {
                                ffi_param_types.push(quote! { &mut dyn std::any::Any });
                            } else {
                                ffi_param_types.push(quote! { &dyn std::any::Any });
                            }
                            handled = true;
                        }
                    }
                }
            }

            // Check for direct &[mut] Object references
            if !handled {
                if let Type::Path(tp) = &*type_ref.elem {
                    if let Some(ident) = tp.path.get_ident() {
                        if is_object_type(&ident.to_string()) {
                            // This is &[mut] SomeObject
                            let generic_name = format_ident!("T{}", i);
                            generic_params.push(quote! { #generic_name: ::hotline::HotlineObject });

                            // Use generic in method signature
                            if type_ref.mutability.is_some() {
                                adjusted_param_types.push(quote! { &mut #generic_name });
                            } else {
                                adjusted_param_types.push(quote! { &#generic_name });
                            }

                            // Use appropriate reference type for FFI
                            if type_ref.mutability.is_some() {
                                ffi_param_types.push(quote! { &mut dyn std::any::Any });
                            } else {
                                ffi_param_types.push(quote! { &dyn std::any::Any });
                            }
                            handled = true;
                        }
                    }
                }
            }
        }

        if !handled {
            // Not an object parameter, keep as-is
            adjusted_param_types.push(quote! { #param_type });
            ffi_param_types.push(quote! { #param_type });
        }
    }

    // Build parameter list for method signature
    let method_params = param_idents.iter().zip(&adjusted_param_types).map(|(name, ty)| quote! { #name: #ty });

    let param_specs: Vec<_> =
        param_names.iter().zip(param_types.iter()).map(|(name, ty)| (name.clone(), type_to_string(ty))).collect();

    let is_mut_receiver = *receiver_type == ReceiverType::RefMut;
    let symbol = SymbolName::new(type_name, &config.method_name, rustc_commit)
        .with_params(param_specs)
        .with_return_type(type_to_string(return_type))
        .with_receiver_mutability(is_mut_receiver);

    let symbol_name = symbol.build_method();

    // Build FFI call arguments
    let ffi_args = param_idents.iter().zip(param_types.iter()).map(|(name, param_type)| {
        if let Type::Reference(type_ref) = param_type {
            // Check for Like<Object>
            if let Some(inner) = extract_like_type(&type_ref.elem) {
                if let Type::Path(tp) = inner {
                    if let Some(ident) = tp.path.get_ident() {
                        if is_object_type(&ident.to_string()) {
                            // Convert to dyn Any for FFI
                            return if type_ref.mutability.is_some() {
                                quote! { #name.as_any_mut() }
                            } else {
                                quote! { #name.as_any() }
                            };
                        }
                    }
                }
            }

            // Check for direct object reference
            if let Type::Path(tp) = &*type_ref.elem {
                if let Some(ident) = tp.path.get_ident() {
                    if is_object_type(&ident.to_string()) {
                        // Convert to dyn Any for FFI
                        return if type_ref.mutability.is_some() {
                            quote! { #name.as_any_mut() }
                        } else {
                            quote! { #name.as_any() }
                        };
                    }
                }
            }
        }
        quote! { #name }
    });

    let receiver_param = if is_mut_receiver {
        quote! { &mut dyn std::any::Any }
    } else {
        quote! { &dyn std::any::Any }
    };

    let fn_type = quote! {
        unsafe extern "Rust" fn(#receiver_param #(, #ffi_param_types)*) -> #return_type
    };

    let method_body = quote_method_call_with_registry(
        quote! { self.0 },
        &method_ident.to_string(),
        &symbol_name,
        fn_type,
        quote! { #(, #ffi_args)* },
        is_mut_receiver,
    );

    // Build generic parameters if any
    let generics = if generic_params.is_empty() {
        quote! {}
    } else {
        quote! { <#(#generic_params),*> }
    };

    if config.returns_self {
        quote! {
            pub fn #method_ident #generics(&mut self #(, #method_params)*) -> Self {
                let handle_clone = self.0.clone();
                #method_body;
                Self::from_handle(handle_clone)
            }
        }
    } else {
        let self_param = if is_mut_receiver {
            quote! { &mut self }
        } else {
            quote! { &self }
        };

        quote! {
            pub fn #method_ident #generics(#self_param #(, #method_params)*) -> #return_type {
                #method_body
            }
        }
    }
}
