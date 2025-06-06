pub mod types {
    use proc_macro_error2::abort_call_site;
    use syn::{GenericArgument, PathArguments, Type};

    pub fn type_to_string(ty: &Type) -> String {
        // Handle Like<T> specially
        if let Some(inner_ty) = extract_like_type(ty) {
            return type_to_string(inner_ty);
        }

        match ty {
            Type::Path(type_path) => path_to_string(type_path),
            Type::Reference(type_ref) => {
                format!(
                    "{}ref_{}",
                    if type_ref.mutability.is_some() { "mut_" } else { "" },
                    type_to_string(&type_ref.elem)
                )
            }
            Type::Slice(type_slice) => format!("slice_{}", type_to_string(&type_slice.elem)),
            Type::Array(type_array) => format!("array_{}", type_to_string(&type_array.elem)),
            Type::Tuple(type_tuple) if type_tuple.elems.is_empty() => "unit".to_string(),
            Type::Tuple(type_tuple) => {
                let elems: Vec<_> = type_tuple.elems.iter().map(type_to_string).collect();
                format!("tuple_{}", elems.join("_comma_"))
            }
            _ => abort_call_site!(format!("Unsupported type in hotline macro: {:?}", ty)),
        }
    }

    pub fn path_to_string(type_path: &syn::TypePath) -> String {
        if let Some(ident) = type_path.path.get_ident() {
            return ident.to_string();
        }

        let mut result = String::new();
        for (i, segment) in type_path.path.segments.iter().enumerate() {
            if i > 0 {
                result.push_str("_");
            }
            result.push_str(&segment.ident.to_string());

            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                result.push_str("_lt_");
                let types: Vec<_> = args
                    .args
                    .iter()
                    .filter_map(|arg| match arg {
                        GenericArgument::Type(ty) => Some(type_to_string(ty)),
                        _ => None,
                    })
                    .collect();
                result.push_str(&types.join("_"));
                result.push_str("_gt");
            }
        }
        result
    }

    pub fn is_standard_type(name: &str) -> bool {
        matches!(
            name,
            "String"
                | "Vec"
                | "Option"
                | "Result"
                | "Box"
                | "Arc"
                | "Mutex"
                | "HashMap"
                | "BTreeMap"
                | "HashSet"
                | "BTreeSet"
                | "Cell"
                | "RefCell"
                | "Rc"
                | "Weak"
                | "PhantomData"
                | "Pin"
                | "Future"
                | "Stream"
        )
    }

    pub fn is_object_type(name: &str) -> bool {
        name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) && !is_standard_type(name)
    }

    pub fn is_primitive_type(name: &str) -> bool {
        matches!(
            name,
            "i8" | "i16"
                | "i32"
                | "i64"
                | "i128"
                | "isize"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "u128"
                | "usize"
                | "f32"
                | "f64"
                | "bool"
                | "char"
                | "str"
        )
    }

    pub fn contains_external_type(ty: &Type) -> bool {
        match ty {
            Type::Path(tp) => {
                if let Some(ident) = tp.path.get_ident() {
                    let name = ident.to_string();
                    // if it's not object, not standard, not primitive, it's external
                    !is_object_type(&name) && !is_standard_type(&name) && !is_primitive_type(&name)
                } else if tp.path.segments.len() > 1 {
                    // multi-segment paths like sdl2::render::Canvas are external
                    true
                } else {
                    false
                }
            }
            Type::Reference(tr) => contains_external_type(&tr.elem),
            Type::Slice(ts) => contains_external_type(&ts.elem),
            Type::Array(ta) => contains_external_type(&ta.elem),
            Type::Tuple(tt) => tt.elems.iter().any(contains_external_type),
            _ => false,
        }
    }

    pub fn contains_reference(ty: &Type) -> bool {
        match ty {
            Type::Reference(_) => true,
            Type::Path(tp) => {
                // Check if it's Option<&T> or similar
                if let Some(seg) = tp.path.segments.last() {
                    if let PathArguments::AngleBracketed(args) = &seg.arguments {
                        return args.args.iter().any(|arg| {
                            if let GenericArgument::Type(inner_ty) = arg { contains_reference(inner_ty) } else { false }
                        });
                    }
                }
                false
            }
            Type::Slice(_) => true, // slices are reference types
            Type::Array(ta) => contains_reference(&ta.elem),
            Type::Tuple(tt) => tt.elems.iter().any(contains_reference),
            _ => false,
        }
    }

    pub fn is_generic_type(ty: &Type) -> bool {
        if let Type::Path(type_path) = ty {
            type_path.path.segments.iter().any(|seg| matches!(seg.arguments, PathArguments::AngleBracketed(_)))
        } else {
            false
        }
    }

    pub fn extract_generic_inner<'a>(ty: &'a Type, type_name: &str) -> Option<&'a Type> {
        match ty {
            Type::Path(tp) => {
                tp.path.segments.last().filter(|seg| seg.ident == type_name).and_then(|seg| match &seg.arguments {
                    PathArguments::AngleBracketed(args) => args.args.first().and_then(|arg| match arg {
                        GenericArgument::Type(inner) => Some(inner),
                        _ => None,
                    }),
                    _ => None,
                })
            }
            _ => None,
        }
    }

    pub fn extract_option_type(ty: &Type) -> Option<&Type> {
        extract_generic_inner(ty, "Option")
    }

    pub fn extract_like_type(ty: &Type) -> Option<&Type> {
        extract_generic_inner(ty, "Like")
    }

    pub fn resolve_self_type(ty: Type, type_name: &str) -> Type {
        use syn::visit_mut::{self, VisitMut};

        struct SelfResolver<'a> {
            type_name: &'a str,
        }

        impl<'a> VisitMut for SelfResolver<'a> {
            fn visit_type_mut(&mut self, ty: &mut Type) {
                if let Type::Path(type_path) = ty {
                    if type_path.path.is_ident("Self") {
                        *ty = syn::parse_str(self.type_name).expect("Failed to parse type name");
                        return;
                    }
                }
                visit_mut::visit_type_mut(self, ty);
            }
        }

        let mut ty = ty;
        SelfResolver { type_name }.visit_type_mut(&mut ty);
        ty
    }
}

pub mod symbols {
    #[derive(Clone)]
    pub struct SymbolName {
        type_name: String,
        method_name: String,
        params: Vec<(String, String)>,
        return_type: String,
        rustc_commit: String,
    }

    impl SymbolName {
        pub fn new(type_name: &str, method_name: &str, rustc_commit: &str) -> Self {
            Self {
                type_name: type_name.to_string(),
                method_name: method_name.to_string(),
                params: Vec::new(),
                return_type: "unit".to_string(),
                rustc_commit: rustc_commit.to_string(),
            }
        }

        pub fn with_params(mut self, params: Vec<(String, String)>) -> Self {
            self.params = params;
            self
        }

        pub fn with_return_type(mut self, return_type: String) -> Self {
            self.return_type = return_type;
            self
        }

        pub fn build_method(&self) -> String {
            let mut parts = vec![self.type_name.clone(), self.method_name.clone(), "____obj_mut_dyn_Any".to_string()];

            for (name, ty) in &self.params {
                parts.push(format!("__{}__{}", name, ty));
            }

            parts.push(format!("__to__{}", self.return_type));
            parts.push(self.rustc_commit.clone());

            parts.join("__")
        }

        pub fn build_getter(&self) -> String {
            format!(
                "{}__{}______obj_mut_dyn_Any____to__{}__{}",
                self.type_name, self.method_name, self.return_type, self.rustc_commit
            )
        }

        pub fn build_constructor(&self) -> String {
            format!("{}__new____to__Box_lt_dyn_HotlineObject_gt__{}", self.type_name, self.rustc_commit)
        }

        pub fn build_type_name_getter(&self) -> String {
            format!("{}__get_type_name____obj_ref_dyn_Any__to__str__{}", self.type_name, self.rustc_commit)
        }
    }
}
