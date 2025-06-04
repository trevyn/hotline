pub mod types {
    use syn::{Type, GenericArgument, PathArguments};
    
    pub fn type_to_string(ty: &Type) -> String {
        // Handle Like<T> specially
        if let Some(inner_ty) = extract_like_type(ty) {
            return type_to_string(inner_ty);
        }
        
        match ty {
            Type::Path(type_path) => path_to_string(type_path),
            Type::Reference(type_ref) => {
                format!("{}ref_{}", 
                    if type_ref.mutability.is_some() { "mut_" } else { "" },
                    type_to_string(&type_ref.elem))
            }
            Type::Slice(type_slice) => format!("slice_{}", type_to_string(&type_slice.elem)),
            Type::Array(type_array) => format!("array_{}", type_to_string(&type_array.elem)),
            Type::Tuple(type_tuple) if type_tuple.elems.is_empty() => "unit".to_string(),
            Type::Tuple(type_tuple) => {
                let elems: Vec<_> = type_tuple.elems.iter()
                    .map(type_to_string)
                    .collect();
                format!("tuple_{}", elems.join("_comma_"))
            }
            _ => panic!("Unsupported type in hotline macro: {:?}", ty),
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
                let types: Vec<_> = args.args.iter()
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
            "String" | "Vec" | "Option" | "Result" | "Box" | "Arc" | "Mutex" | 
            "HashMap" | "BTreeMap" | "HashSet" | "BTreeSet" | "Cell" | "RefCell" | 
            "Rc" | "Weak" | "PhantomData" | "Pin" | "Future" | "Stream"
        )
    }
    
    pub fn is_object_type(name: &str) -> bool {
        name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) && !is_standard_type(name)
    }
    
    pub fn is_generic_type(ty: &Type) -> bool {
        if let Type::Path(type_path) = ty {
            type_path.path.segments.iter().any(|seg| 
                matches!(seg.arguments, PathArguments::AngleBracketed(_)))
        } else {
            false
        }
    }
    
    pub fn extract_generic_inner_type<'a>(ty: &'a Type, type_name: &str) -> Option<&'a Type> {
        if let Type::Path(type_path) = ty {
            if let Some(segment) = type_path.path.segments.last() {
                if segment.ident == type_name {
                    if let PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                            return Some(inner_ty);
                        }
                    }
                }
            }
        }
        None
    }
    
    pub fn extract_option_type(ty: &Type) -> Option<&Type> {
        extract_generic_inner_type(ty, "Option")
    }
    
    pub fn extract_like_type(ty: &Type) -> Option<&Type> {
        extract_generic_inner_type(ty, "Like")
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
            let mut parts = vec![
                self.type_name.clone(),
                self.method_name.clone(),
                "____obj_mut_dyn_Any".to_string(),
            ];
            
            for (name, ty) in &self.params {
                parts.push(format!("__{}__{}", name, ty));
            }
            
            parts.push(format!("__to__{}", self.return_type));
            parts.push(self.rustc_commit.clone());
            
            parts.join("__")
        }
    
        pub fn build_getter(&self) -> String {
            format!("{}__{}______obj_mut_dyn_Any____to__{}__{}",
                self.type_name, self.method_name, self.return_type, self.rustc_commit)
        }
    
        pub fn build_setter(&self, field_name: &str, field_type: &str) -> String {
            format!("{}__set_{}____obj_mut_dyn_Any__{}_{}__to__unit__{}",
                self.type_name, field_name, field_name, field_type, self.rustc_commit)
        }
    
        pub fn build_constructor(&self) -> String {
            format!("{}__new____to__Box_lt_dyn_HotlineObject_gt__{}", self.type_name, self.rustc_commit)
        }
    
        pub fn build_type_name_getter(&self) -> String {
            format!("{}__get_type_name____obj_ref_dyn_Any__to__str__{}", self.type_name, self.rustc_commit)
        }
    
        pub fn build_init(&self) -> String {
            format!("{}__init__registry__{}", self.type_name, self.rustc_commit)
        }
    }
}