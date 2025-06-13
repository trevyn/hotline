use std::collections::HashSet;
use std::path::PathBuf;
use syn::{
    ItemImpl, ItemStruct, Type,
    visit::{self, Visit},
};

use crate::parser::ObjectInput;
use crate::utils::types::{extract_like_type, is_object_type};

pub fn find_referenced_object_types(struct_item: &ItemStruct, impl_blocks: &[ItemImpl]) -> HashSet<String> {
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

pub fn find_referenced_custom_types(struct_item: &ItemStruct, impl_blocks: &[ItemImpl]) -> HashSet<String> {
    struct CustomTypeVisitor {
        types: HashSet<String>,
        current_type: String,
    }

    impl<'ast> Visit<'ast> for CustomTypeVisitor {
        fn visit_type(&mut self, ty: &'ast Type) {
            match ty {
                Type::Path(type_path) => {
                    if let Some(ident) = type_path.path.get_ident() {
                        let name = ident.to_string();
                        // Look for custom types that might be shared (RenderCommand, AtlasFormat, etc)
                        if name != self.current_type && name != "Self" 
                            && !name.starts_with(char::is_lowercase) // Skip primitive types
                            && name != "String" && name != "Vec" && name != "Option" && name != "Result"
                        {
                            self.types.insert(name);
                        }
                    }
                }
                Type::Reference(type_ref) => {
                    // Also check types inside references
                    self.visit_type(&type_ref.elem);
                }
                Type::Slice(type_slice) => {
                    // Also check types inside slices
                    self.visit_type(&type_slice.elem);
                }
                _ => {}
            }
            visit::visit_type(self, ty);
        }

        fn visit_impl_item_fn(&mut self, method: &'ast syn::ImplItemFn) {
            // Visit method parameters
            for input in &method.sig.inputs {
                if let syn::FnArg::Typed(pat_type) = input {
                    self.visit_type(&pat_type.ty);
                }
            }

            // Visit return type
            if let syn::ReturnType::Type(_, ty) = &method.sig.output {
                self.visit_type(ty);
            }

            // Continue with default visitation (for method body)
            visit::visit_impl_item_fn(self, method);
        }

        fn visit_expr(&mut self, expr: &'ast syn::Expr) {
            match expr {
                syn::Expr::Struct(expr_struct) => {
                    // Handle simple struct construction: MyStruct { ... }
                    if let Some(ident) = expr_struct.path.get_ident() {
                        let name = ident.to_string();
                        if name != self.current_type && !name.starts_with(char::is_lowercase) {
                            self.types.insert(name);
                        }
                    }
                    // Handle enum variant construction: RenderCommand::Atlas { ... }
                    else if expr_struct.path.segments.len() > 1 {
                        let type_name = expr_struct.path.segments[0].ident.to_string();
                        if !type_name.starts_with(char::is_lowercase) {
                            self.types.insert(type_name);
                        }
                    }
                }
                syn::Expr::Path(path_expr) => {
                    // Handle type references: AtlasFormat::GrayscaleAlpha
                    if path_expr.path.segments.len() > 1 {
                        let type_name = path_expr.path.segments[0].ident.to_string();
                        if !type_name.starts_with(char::is_lowercase) {
                            self.types.insert(type_name);
                        }
                    }
                }
                _ => {}
            }

            visit::visit_expr(self, expr);
        }
    }

    let mut visitor = CustomTypeVisitor { types: HashSet::new(), current_type: struct_item.ident.to_string() };
    visitor.visit_item_struct(struct_item);
    impl_blocks.iter().for_each(|block| visitor.visit_item_impl(block));
    visitor.types
}

pub fn find_object_lib_file(type_name: &str) -> PathBuf {
    let find_workspace = |start: PathBuf| -> Option<PathBuf> {
        start.ancestors().find(|d| d.join("Cargo.toml").exists() && d.join("objects").exists()).map(PathBuf::from)
    };

    let workspace = std::env::current_dir()
        .ok()
        .and_then(find_workspace)
        .or_else(|| std::env::var("OUT_DIR").ok().and_then(|s| find_workspace(s.into())))
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    workspace.join("objects").join(type_name).join("src").join("lib.rs")
}

pub fn extract_object_methods(
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReceiverType {
    Value,
    Ref,
    RefMut,
}

fn is_object_macro(path: &syn::Path) -> bool {
    path.is_ident("object")
        || (path.segments.len() == 2 && path.segments[0].ident == "hotline" && path.segments[1].ident == "object")
}

fn extract_methods_from_object(
    obj_input: &ObjectInput,
    type_name: &str,
) -> Vec<(String, Vec<String>, Vec<Type>, Type, ReceiverType)> {
    use crate::utils::types::{extract_option_type, is_generic_type};
    use syn::Fields;

    let mut methods = Vec::new();
    let return_type: Type = syn::parse_str(type_name).unwrap();

    // Extract field getters and builders
    if let Fields::Named(fields) = &obj_input.struct_item.fields {
        for field in &fields.named {
            let field_name = field.ident.as_ref().unwrap();
            let field_type = &field.ty;

            if matches!(field.vis, syn::Visibility::Public(_)) && !is_generic_type(field_type) {
                methods.push((field_name.to_string(), vec![], vec![], field_type.clone(), ReceiverType::Ref));
            }

            if field.attrs.iter().any(|attr| attr.path().is_ident("setter")) {
                // Extract param type once for both methods
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

                // Add setter method (set_*)
                let setter_name = format!("set_{}", field_name);
                methods.push((
                    setter_name,
                    vec!["value".to_string()], // Use "value" to match FFI wrapper generation
                    param_vec.clone(),
                    syn::parse_quote! { () }, // setters return unit
                    ReceiverType::RefMut,
                ));

                // Add builder method (with_*)
                let builder_name = format!("with_{}", field_name);
                methods.push((
                    builder_name,
                    vec!["value".to_string()], // Use "value" to match FFI wrapper generation
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
    use crate::utils::types::{contains_external_type, contains_reference, resolve_self_type};
    use syn::{FnArg, ImplItem, Pat, ReturnType};

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

                // Skip methods with external types
                if param_types.iter().any(contains_external_type) || contains_external_type(&return_type) {
                    return None;
                }

                // Skip methods that return references (can't proxy them across FFI)
                if contains_reference(&return_type) {
                    return None;
                }

                Some((method.sig.ident.to_string(), param_names, param_types, return_type, receiver_type))
            }
            _ => None,
        })
        .collect()
}
