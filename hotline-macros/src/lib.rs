use proc_macro::TokenStream;
use proc_macro_error2::proc_macro_error;
use quote::{ToTokens, quote};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::process::Command;
use syn::{
    Fields, FnArg, ImplItem, ItemImpl, ItemStruct, Pat, ReturnType, Type, braced,
    parse::{Parse, ParseStream},
    visit::{self, Visit},
};

mod utils;
use utils::symbols::SymbolName;
use utils::types::*;

// ===== Core Types =====

struct ObjectInput {
    struct_item: ItemStruct,
    impl_blocks: Vec<ItemImpl>,
}

impl Parse for ObjectInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        braced!(content in input);

        let struct_item: ItemStruct =
            content.parse().map_err(|e| syn::Error::new(e.span(), "Expected a struct definition"))?;

        let mut impl_blocks = Vec::new();
        while !content.is_empty() {
            impl_blocks.push(content.parse::<ItemImpl>()?);
        }

        if impl_blocks.is_empty() {
            return Err(syn::Error::new(content.span(), "Expected at least one impl block after the struct"));
        }

        Ok(ObjectInput { struct_item, impl_blocks })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ReceiverType {
    Value,
    Ref,
    RefMut,
}

// ===== Type Discovery =====

fn find_referenced_object_types(struct_item: &ItemStruct, impl_blocks: &[ItemImpl]) -> HashSet<String> {
    struct TypeVisitor {
        types: HashSet<String>,
        current_type: String,
    }

    impl<'ast> Visit<'ast> for TypeVisitor {
        fn visit_type(&mut self, ty: &'ast Type) {
            if let Some(inner_ty) = extract_like_type(ty) {
                self.visit_type(inner_ty);
                return;
            }

            if let Type::Path(type_path) = ty {
                if let Some(ident) = type_path.path.get_ident() {
                    let name = ident.to_string();
                    if is_object_type(&name) && name != self.current_type && name != "Self" {
                        self.types.insert(name);
                    }
                }
            }
            visit::visit_type(self, ty);
        }

        fn visit_expr(&mut self, expr: &'ast syn::Expr) {
            match expr {
                syn::Expr::Lit(expr_lit) => {
                    if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                        let value = lit_str.value();
                        if is_object_type(&value) && value != self.current_type {
                            self.types.insert(value);
                        }
                    }
                }
                syn::Expr::Call(expr_call) => {
                    if let syn::Expr::Path(path_expr) = &*expr_call.func {
                        if path_expr.path.segments.len() == 2 && path_expr.path.segments[1].ident == "new" {
                            let type_name = path_expr.path.segments[0].ident.to_string();
                            if is_object_type(&type_name) && type_name != self.current_type {
                                self.types.insert(type_name);
                            }
                        }
                    }
                }
                _ => {}
            }
            visit::visit_expr(self, expr);
        }
    }

    let mut visitor = TypeVisitor { types: HashSet::new(), current_type: struct_item.ident.to_string() };

    visitor.visit_item_struct(struct_item);
    impl_blocks.iter().for_each(|block| visitor.visit_item_impl(block));

    visitor.types
}

fn find_object_lib_file(type_name: &str) -> std::path::PathBuf {
    let workspace_dir = std::env::current_dir()
        .ok()
        .and_then(|mut dir| {
            loop {
                if dir.join("Cargo.toml").exists() && dir.join("objects").exists() {
                    return Some(dir);
                }
                if !dir.pop() {
                    break;
                }
            }
            None
        })
        .or_else(|| {
            std::env::var("OUT_DIR").ok().and_then(|out_dir| {
                Path::new(&out_dir)
                    .ancestors()
                    .find(|p| p.ends_with("target"))
                    .and_then(|target| target.parent())
                    .map(|p| p.to_path_buf())
            })
        })
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    workspace_dir.join("objects").join(type_name).join("src").join("lib.rs")
}

// ===== Code Generation Helpers =====

fn quote_method_call_with_registry(
    receiver: proc_macro2::TokenStream,
    method_name: &str,
    symbol_name: &str,
    fn_type: proc_macro2::TokenStream,
    args: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        with_library_registry(|registry| {
            if let Ok(mut guard) = #receiver.lock() {
                let type_name = guard.type_name().to_string();
                let lib_name = format!("lib{}", type_name);
                let obj_any = guard.as_any_mut();

                type FnType = #fn_type;
                registry.with_symbol::<FnType, _, _>(
                    &lib_name,
                    &#symbol_name,
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any #args) }
                ).unwrap_or_else(|e| panic!("Target object {} doesn't have {} method: {}", type_name, #method_name, e))
            } else {
                panic!("Failed to lock object for method {}", #method_name)
            }
        }).unwrap_or_else(|| panic!("No library registry available for method {}", #method_name))
    }
}

fn generate_ffi_wrapper(
    struct_name: &syn::Ident,
    wrapper_name: syn::Ident,
    params: Vec<(syn::Ident, &Type)>,
    return_type: Option<&Type>,
    body: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let param_list = params.iter().map(|(name, ty)| quote! { #name: #ty });
    let return_spec = return_type.map(|ty| quote! { -> #ty }).unwrap_or_default();
    let panic_msg = format!("Type mismatch in {}: expected {}, but got {{}}", wrapper_name, struct_name);

    quote! {
        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub extern "Rust" fn #wrapper_name(
            obj: &mut dyn ::std::any::Any
            #(, #param_list)*
        ) #return_spec {
            let obj_type_name = ::std::any::type_name_of_val(&*obj);
            let instance = obj.downcast_mut::<#struct_name>()
                .unwrap_or_else(|| panic!(#panic_msg, obj_type_name));
            #body
        }
    }
}

fn generate_accessor_wrapper(
    struct_name: &syn::Ident,
    field_name: &syn::Ident,
    field_type: &Type,
    is_getter: bool,
    rustc_commit: &str,
) -> proc_macro2::TokenStream {
    let symbol = SymbolName::new(&struct_name.to_string(), &field_name.to_string(), rustc_commit);
    let type_str = type_to_string(field_type);

    let (wrapper_name, params, return_type, body) = if is_getter {
        let name = quote::format_ident!("{}", symbol.with_return_type(type_str).build_getter());
        (name, vec![], Some(field_type), quote! { instance.#field_name.clone() })
    } else {
        let name = quote::format_ident!("{}", symbol.build_setter(&field_name.to_string(), &type_str));
        let value_ident = quote::format_ident!("value");
        (name, vec![(value_ident, field_type)], None, quote! { instance.#field_name = value; })
    };

    generate_ffi_wrapper(struct_name, wrapper_name, params, return_type, body)
}

// ===== Component Generators =====

struct ProcessedStruct {
    modified_struct: ItemStruct,
    fields_with_setters: HashSet<String>,
    field_defaults: HashMap<String, syn::Expr>,
}

fn process_struct_attributes(struct_item: &ItemStruct) -> ProcessedStruct {
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

fn generate_field_accessors(
    struct_name: &syn::Ident,
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

fn generate_method_wrappers(
    struct_name: &syn::Ident,
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
                    let setter = quote::format_ident!("set_{}", field_name);
                    let type_str = type_to_string(inner_type);
                    let wrapper_name = quote::format_ident!(
                        "{}__{}______obj_mut_dyn_Any____value__ref_{}____to__unit__{}",
                        struct_name,
                        setter,
                        type_str,
                        rustc_commit
                    );
                    generate_ffi_wrapper(
                        struct_name,
                        wrapper_name,
                        vec![(quote::format_ident!("value"), &syn::parse_quote! { &#inner_type })],
                        None,
                        quote! { instance.#setter(value) },
                    )
                })
        }));
    }

    // Generate method wrappers
    wrappers.extend(main_impl.items.iter().filter_map(|item| match item {
        ImplItem::Fn(method) if matches!(method.vis, syn::Visibility::Public(_)) => {
            // Check receiver
            match method.sig.inputs.first()? {
                FnArg::Receiver(r) if r.reference.is_some() => {}
                _ => return None,
            }

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
                ReturnType::Type(_, ty) => Some(resolve_self_type((**ty).clone(), &struct_name.to_string())),
            };

            let symbol = SymbolName::new(&struct_name.to_string(), &method_name.to_string(), rustc_commit)
                .with_params(param_specs)
                .with_return_type(return_type.as_ref().map(type_to_string).unwrap_or_else(|| "unit".to_string()));

            let wrapper_name = quote::format_ident!("{}", symbol.build_method());
            let arg_names: Vec<_> = params.iter().map(|(name, _)| name.clone()).collect();
            let param_vec: Vec<_> = params.clone();

            Some(generate_ffi_wrapper(
                struct_name,
                wrapper_name,
                param_vec,
                return_type.as_ref(),
                quote! { instance.#method_name(#(#arg_names),*) },
            ))
        }
        _ => None,
    }));

    wrappers
}

fn generate_core_functions(
    struct_name: &syn::Ident,
    rustc_commit: &str,
    has_default: bool,
) -> proc_macro2::TokenStream {
    let symbol = SymbolName::new(&struct_name.to_string(), "", rustc_commit);
    let type_name_fn = quote::format_ident!("{}", symbol.build_type_name_getter());
    let init_fn = quote::format_ident!("{}", symbol.build_init());

    let constructor = has_default
        .then(|| {
            let ctor_name = quote::format_ident!("{}", symbol.build_constructor());
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
                    "Type mismatch in type_name getter: expected {}, but got {}",
                    stringify!(#struct_name),
                    ::std::any::type_name_of_val(obj)
                ))
        }

        static mut LIBRARY_REGISTRY: Option<*const ::hotline::LibraryRegistry> = None;

        #[unsafe(no_mangle)]
        #[allow(non_snake_case)]
        pub extern "C" fn #init_fn(registry: *const ::hotline::LibraryRegistry) {
            unsafe { LIBRARY_REGISTRY = Some(registry); }
        }

        pub fn with_library_registry<F, R>(f: F) -> Option<R>
        where F: FnOnce(&::hotline::LibraryRegistry) -> R,
        {
            unsafe { LIBRARY_REGISTRY.and_then(|ptr| ptr.as_ref()).map(f) }
        }
    }
}

fn generate_setter_builder_methods(struct_name: &syn::Ident, processed: &ProcessedStruct) -> proc_macro2::TokenStream {
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
                let setter = quote::format_ident!("set_{}", field_name);
                let builder = quote::format_ident!("with_{}", field_name);

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

fn generate_default_impl(struct_name: &syn::Ident, processed: &ProcessedStruct) -> proc_macro2::TokenStream {
    if processed.field_defaults.is_empty() {
        return quote! {};
    }

    if let Fields::Named(fields) = &processed.modified_struct.fields {
        let field_inits: Vec<_> = fields
            .named
            .iter()
            .filter_map(|field| {
                let field_name = field.ident.as_ref()?;
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
                    Self { #(#field_inits),* }
                }
            }
        }
    } else {
        quote! {}
    }
}

// ===== Type Wrapper Generation =====

#[derive(Debug)]
struct MethodGenConfig {
    method_name: String,
    method_ident: syn::Ident,
    param_names: Vec<String>,
    param_idents: Vec<syn::Ident>,
    param_types: Vec<Type>,
    return_type: Type,
    receiver_type: ReceiverType,
    returns_self: bool,
    is_builder: bool,
    needs_option_wrap: bool,
    setter_name: Option<String>,
}

impl MethodGenConfig {
    fn should_skip(&self) -> bool {
        self.receiver_type == ReceiverType::Value && !self.returns_self
    }

    fn is_field_setter(&self) -> bool {
        self.is_builder && self.setter_name.is_some() && self.method_name.starts_with("with_")
    }
}

fn generate_method_impl(config: &MethodGenConfig, type_name: &str, rustc_commit: &str) -> proc_macro2::TokenStream {
    let MethodGenConfig { method_ident, param_idents, param_types, return_type, .. } = config;

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
            let field_name = &config.setter_name.as_ref().unwrap()[4..];
            let param_type_str = type_to_string(&param_types[0]);
            format!(
                "{}__set_{}____obj_mut_dyn_Any__{}_{}__to__unit__{}",
                type_name, field_name, field_name, param_type_str, rustc_commit
            )
        };

        return quote! {
            pub fn #method_ident(mut self #(, #param_idents: #param_types)*) -> Self {
                with_library_registry(|registry| {
                    if let Ok(mut guard) = self.0.lock() {
                        let type_name = guard.type_name().to_string();
                        let lib_name = format!("lib{}", type_name);
                        let obj_any = guard.as_any_mut();

                        type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any #(, #param_types)*);
                        registry.with_symbol::<FnType, _, _>(&lib_name, &#symbol_name,
                            |fn_ptr| unsafe { (**fn_ptr)(obj_any #(, #param_idents)*) }
                        ).unwrap_or_else(|e| panic!("Method not found: {}", e));
                    }
                });
                self
            }
        };
    }

    // Regular methods
    let param_specs: Vec<_> = config
        .param_names
        .iter()
        .zip(param_types.iter())
        .map(|(name, ty)| (name.clone(), type_to_string(ty)))
        .collect();

    let symbol = SymbolName::new(type_name, &config.method_name, rustc_commit)
        .with_params(param_specs)
        .with_return_type(type_to_string(return_type));

    let symbol_name = symbol.build_method();
    let fn_type = quote! {
        unsafe extern "Rust" fn(&mut dyn std::any::Any #(, #param_types)*) -> #return_type
    };

    let method_body = quote_method_call_with_registry(
        quote! { self.0 },
        &method_ident.to_string(),
        &symbol_name,
        fn_type,
        quote! { #(, #param_idents)* },
    );

    if config.returns_self {
        quote! {
            pub fn #method_ident(&mut self #(, #param_idents: #param_types)*) -> Self {
                let handle_clone = self.0.clone();
                #method_body;
                Self::from_handle(handle_clone)
            }
        }
    } else {
        quote! {
            pub fn #method_ident(&mut self #(, #param_idents: #param_types)*) -> #return_type {
                #method_body
            }
        }
    }
}

fn extract_object_methods(
    file: &syn::File,
    type_name: &str,
) -> Option<Vec<(String, Vec<String>, Vec<Type>, Type, ReceiverType)>> {
    file.items.iter().find_map(|item| match item {
        syn::Item::Macro(item_macro) if is_object_macro(&item_macro.mac.path) => {
            syn::parse2::<ObjectInput>(item_macro.mac.tokens.clone())
                .ok()
                .map(|obj_input| extract_methods_from_object(&obj_input, type_name))
        }
        _ => None,
    })
}

fn is_object_macro(path: &syn::Path) -> bool {
    path.is_ident("object")
        || (path.segments.len() == 2 && path.segments[0].ident == "hotline" && path.segments[1].ident == "object")
}

fn extract_methods_from_object(
    obj_input: &ObjectInput,
    type_name: &str,
) -> Vec<(String, Vec<String>, Vec<Type>, Type, ReceiverType)> {
    let mut methods = Vec::new();
    let return_type: Type = syn::parse_str(type_name).unwrap();

    // Extract field getters and builders
    if let Fields::Named(fields) = &obj_input.struct_item.fields {
        for field in &fields.named {
            let field_name = field.ident.as_ref().unwrap();
            let field_type = &field.ty;

            if matches!(field.vis, syn::Visibility::Public(_)) && !is_generic_type(field_type) {
                methods.push((field_name.to_string(), vec![], vec![], field_type.clone(), ReceiverType::RefMut));
            }

            if field.attrs.iter().any(|attr| attr.path().is_ident("setter")) {
                let builder_name = format!("with_{}", field_name);
                let param_vec = extract_option_type(field_type)
                    .and_then(|inner| match inner {
                        Type::Path(tp)
                            if tp.path.get_ident().map(|i| is_object_type(&i.to_string())).unwrap_or(false) =>
                        {
                            Some(vec![syn::parse_quote! { &#inner }])
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| vec![field_type.clone()]);

                methods.push((
                    builder_name,
                    vec![field_name.to_string()],
                    param_vec,
                    return_type.clone(),
                    ReceiverType::Value,
                ));
            }
        }
    }

    // Extract impl methods
    if let Some(main_impl) = find_main_impl(&obj_input.impl_blocks, type_name) {
        methods.extend(extract_impl_methods(main_impl, type_name));
    }

    methods
}

fn find_main_impl<'a>(impl_blocks: &'a [ItemImpl], type_name: &str) -> Option<&'a ItemImpl> {
    impl_blocks.iter().find(|impl_block| {
        impl_block.trait_.is_none()
            && match &*impl_block.self_ty {
                Type::Path(tp) => tp.path.get_ident().map(|i| i.to_string() == type_name).unwrap_or(false),
                _ => false,
            }
    })
}

fn extract_impl_methods(
    main_impl: &ItemImpl,
    type_name: &str,
) -> Vec<(String, Vec<String>, Vec<Type>, Type, ReceiverType)> {
    main_impl
        .items
        .iter()
        .filter_map(|item| match item {
            ImplItem::Fn(method) if matches!(method.vis, syn::Visibility::Public(_)) => {
                let receiver_type = match method.sig.inputs.first()? {
                    FnArg::Receiver(r) if r.reference.is_none() => ReceiverType::Value,
                    FnArg::Receiver(r) if r.mutability.is_some() => ReceiverType::RefMut,
                    FnArg::Receiver(_) => ReceiverType::Ref,
                    _ => return None,
                };

                let (param_names, param_types): (Vec<_>, Vec<_>) = method
                    .sig
                    .inputs
                    .iter()
                    .skip(1)
                    .filter_map(|arg| match arg {
                        FnArg::Typed(typed) => match &*typed.pat {
                            Pat::Ident(pat_ident) => Some((pat_ident.ident.to_string(), (*typed.ty).clone())),
                            _ => None,
                        },
                        _ => None,
                    })
                    .unzip();

                let return_type = match &method.sig.output {
                    ReturnType::Default => syn::parse_quote! { () },
                    ReturnType::Type(_, ty) => resolve_self_type((**ty).clone(), type_name),
                };

                Some((method.sig.ident.to_string(), param_names, param_types, return_type, receiver_type))
            }
            _ => None,
        })
        .collect()
}

fn generate_typed_wrapper(
    type_name: &str,
    methods: &[(String, Vec<String>, Vec<Type>, Type, ReceiverType)],
    rustc_commit: &str,
) -> proc_macro2::TokenStream {
    let type_ident = quote::format_ident!("{}", type_name);

    let method_impls: Vec<_> = methods
        .iter()
        .filter_map(|(method_name, param_names, param_types, return_type, receiver_type)| {
            let method_ident = quote::format_ident!("{}", method_name);
            let param_idents: Vec<_> = param_names.iter().map(|name| quote::format_ident!("{}", name)).collect();

            let returns_self = matches!(return_type, Type::Path(tp) if tp.path.is_ident(type_name));
            let is_builder = *receiver_type == ReceiverType::Value && returns_self;
            let setter_name = method_name.strip_prefix("with_").map(|s| format!("set_{}", s));

            let needs_option_wrap = method_name.starts_with("with_") && param_types.len() == 1 && {
                let inner = match &param_types[0] {
                    Type::Reference(tr) => &*tr.elem,
                    t => t,
                };
                matches!(inner, Type::Path(tp) if tp.path.get_ident()
                    .map(|i| is_object_type(&i.to_string()))
                    .unwrap_or(false))
            };

            let config = MethodGenConfig {
                method_name: method_name.clone(),
                method_ident,
                param_names: param_names.clone(),
                param_idents,
                param_types: param_types.clone(),
                return_type: return_type.clone(),
                receiver_type: *receiver_type,
                returns_self,
                is_builder,
                needs_option_wrap,
                setter_name,
            };

            let impl_tokens = generate_method_impl(&config, type_name, rustc_commit);
            (!impl_tokens.is_empty()).then_some(impl_tokens)
        })
        .collect();

    quote! {
        #[derive(Clone)]
        pub struct #type_ident(::hotline::ObjectHandle);

        impl #type_ident {
            pub fn new() -> Self {
                let obj = with_library_registry(|registry| {
                    registry.call_constructor(concat!("lib", #type_name), #type_name, ::hotline::RUSTC_COMMIT)
                        .expect(&format!("failed to construct {}", #type_name))
                }).expect("library registry not initialized");

                Self::from_handle(::std::sync::Arc::new(::std::sync::Mutex::new(obj)))
            }

            pub fn from_handle(handle: ::hotline::ObjectHandle) -> Self {
                Self(handle)
            }

            pub fn handle(&self) -> &::hotline::ObjectHandle {
                &self.0
            }

            #(#method_impls)*
        }

        impl ::std::ops::Deref for #type_ident {
            type Target = ::hotline::ObjectHandle;
            fn deref(&self) -> &Self::Target { &self.0 }
        }

        impl ::std::ops::DerefMut for #type_ident {
            fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
        }
    }
}

fn generate_typed_wrappers(types: &HashSet<String>, rustc_commit: &str) -> proc_macro2::TokenStream {
    let wrappers: Vec<_> = types
        .iter()
        .filter_map(|type_name| {
            let lib_path = find_object_lib_file(type_name);
            fs::read_to_string(&lib_path)
                .ok()
                .and_then(|content| syn::parse_file(&content).ok())
                .and_then(|file| extract_object_methods(&file, type_name))
                .map(|methods| generate_typed_wrapper(type_name, &methods, rustc_commit))
        })
        .collect();

    quote! { #(#wrappers)* }
}

// ===== Main Macro =====

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
