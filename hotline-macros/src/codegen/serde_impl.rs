use proc_macro2::TokenStream;
use quote::quote;
use syn::{Fields, Type};

use crate::codegen::ProcessedStruct;

pub fn generate_state_serialization(struct_name: &syn::Ident, processed: &ProcessedStruct) -> TokenStream {
    // Generate a separate serializable struct that excludes __hotline_registry
    let state_struct_name = syn::Ident::new(&format!("{}State", struct_name), struct_name.span());

    let fields = if let Fields::Named(ref fields) = processed.modified_struct.fields {
        fields
            .named
            .iter()
            .filter(|f| {
                // Skip internal fields, trait objects, and non-serializable types
                f.ident.as_ref().map(|i| !i.to_string().starts_with("__hotline_")).unwrap_or(true)
                    && !contains_trait_object(&f.ty)
                    && !contains_non_serializable_type(&f.ty)
            })
            .map(|f| {
                let field_name = &f.ident;
                let field_type = &f.ty;

                // Preserve existing serde attributes from the original field
                let serde_attrs: Vec<_> = f.attrs.iter().filter(|attr| attr.path().is_ident("serde")).collect();

                // Check if this is an object reference field
                if is_option_object_wrapper_type(&f.ty) {
                    // Use custom serialization for Option<Object> types where Object is a wrapper
                    quote! {
                        #(#serde_attrs)*
                        #[serde(with = "::hotline::object_serde::option_object")]
                        #field_name: #field_type
                    }
                } else if is_object_wrapper_type(&f.ty) {
                    // For direct object references (not wrapped in Option) that are wrappers
                    quote! {
                        #(#serde_attrs)*
                        #[serde(serialize_with = "::hotline::object_serde::serialize_object_handle")]
                        #[serde(deserialize_with = "::hotline::object_serde::deserialize_object_handle")]
                        #field_name: #field_type
                    }
                } else {
                    quote! {
                        #(#serde_attrs)*
                        #field_name: #field_type
                    }
                }
            })
            .collect::<Vec<_>>()
    } else {
        vec![]
    };

    let field_copies = if let Fields::Named(ref fields) = processed.modified_struct.fields {
        fields
            .named
            .iter()
            .filter(|f| {
                // Skip internal fields, trait objects, and non-serializable types
                f.ident.as_ref().map(|i| !i.to_string().starts_with("__hotline_")).unwrap_or(true)
                    && !contains_trait_object(&f.ty)
                    && !contains_non_serializable_type(&f.ty)
            })
            .map(|f| {
                let field_name = &f.ident;
                if is_option_object_type(&f.ty) || is_object_type(&f.ty) {
                    // For object references, just copy the reference
                    quote! {
                        #field_name: self.#field_name.clone()
                    }
                } else {
                    quote! {
                        #field_name: self.#field_name.clone()
                    }
                }
            })
            .collect::<Vec<_>>()
    } else {
        vec![]
    };

    let field_restores = if let Fields::Named(ref fields) = processed.modified_struct.fields {
        fields
            .named
            .iter()
            .filter(|f| {
                // Skip internal fields, trait objects, and non-serializable types
                f.ident.as_ref().map(|i| !i.to_string().starts_with("__hotline_")).unwrap_or(true)
                    && !contains_trait_object(&f.ty)
                    && !contains_non_serializable_type(&f.ty)
            })
            .map(|f| {
                let field_name = &f.ident;
                quote! {
                    self.#field_name = state.#field_name;
                }
            })
            .collect::<Vec<_>>()
    } else {
        vec![]
    };

    quote! {
        #[derive(::hotline::serde::Serialize, ::hotline::serde::Deserialize)]
        #[serde(crate = "::hotline::serde")]
        struct #state_struct_name {
            #(#fields,)*
        }

        impl #struct_name {
            fn __serialize_state_impl(&self) -> Result<Vec<u8>, String> {
                let state = #state_struct_name {
                    #(#field_copies,)*
                };
                ::hotline::serde_json::to_vec(&state).map_err(|e| e.to_string())
            }

            fn __deserialize_state_impl(&mut self, data: &[u8]) -> Result<(), String> {
                let state: #state_struct_name = ::hotline::serde_json::from_slice(data)
                    .map_err(|e| e.to_string())?;
                #(#field_restores)*
                Ok(())
            }
        }
    }
}

pub fn generate_migrate_children_impl(struct_name: &syn::Ident, processed: &ProcessedStruct) -> TokenStream {
    let field_migrations = if let Fields::Named(ref fields) = processed.modified_struct.fields {
        fields
            .named
            .iter()
            .filter_map(|f| {
                let field_name = &f.ident;

                // Skip the registry field
                if field_name.as_ref().map(|i| i == "__hotline_registry").unwrap_or(false) {
                    return None;
                }

                // Check if this is an object handle field
                if is_object_handle_type(&f.ty) {
                    Some(generate_field_migration(field_name.as_ref().unwrap(), &f.ty))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    } else {
        vec![]
    };

    quote! {
        impl #struct_name {
            fn migrate_children_impl(&mut self, reloaded_libs: &::std::collections::HashSet<String>) -> Result<(), String> {
                #(#field_migrations)*
                Ok(())
            }
        }
    }
}

fn generate_field_migration(field_name: &syn::Ident, field_type: &Type) -> TokenStream {
    // Handle Option<T> where T is an object type
    if let Type::Path(type_path) = field_type {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                return quote! {
                    if let Some(ref handle) = self.#field_name {
                        if let Ok(mut guard) = handle.lock() {
                            let type_name = guard.type_name();
                            if reloaded_libs.contains(type_name) {
                                // Serialize old object
                                let data = guard.serialize_state()?;

                                // Create new using existing infrastructure
                                if let Some(registry) = self.get_registry() {
                                    let mut new_obj = registry.call_constructor(
                                        &format!("lib{}", type_name),
                                        type_name,
                                        ::hotline::RUSTC_COMMIT
                                    ).map_err(|e| e.to_string())?;

                                    // Restore state
                                    new_obj.deserialize_state(&data)?;
                                    new_obj.set_registry(registry);

                                    // Swap inside mutex
                                    *guard = new_obj;
                                }
                            }
                            // Recurse
                            guard.migrate_children(reloaded_libs)?;
                        }
                    }
                };
            }

            // Handle Vec<T> where T is an object type
            if segment.ident == "Vec" {
                // Check if inner type is an object wrapper type
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        if is_object_wrapper_type(inner_ty) {
                            // Vec<ObjectWrapper> - handle as wrapped objects
                            return quote! {
                                // Get registry once before the loop
                                if let Some(registry) = self.get_registry() {
                                    // Migrate all object handles in the vector
                                    for handle in &mut self.#field_name {
                                        if let Ok(mut guard) = handle.lock() {
                                            let type_name = guard.type_name();
                                            if reloaded_libs.contains(type_name) {
                                                // Serialize old object
                                                let data = guard.serialize_state()?;

                                                // Create new using existing infrastructure
                                                let mut new_obj = registry.call_constructor(
                                                    &format!("lib{}", type_name),
                                                    type_name,
                                                    ::hotline::RUSTC_COMMIT
                                                ).map_err(|e| e.to_string())?;

                                                // Restore state
                                                new_obj.deserialize_state(&data)?;
                                                new_obj.set_registry(registry);

                                                // Swap inside mutex
                                                *guard = new_obj;
                                            }
                                            // Recurse
                                            guard.migrate_children(reloaded_libs)?;
                                        }
                                    }
                                }
                            };
                        } else if is_object_type(inner_ty) {
                            // Vec<T> where T is an object type - these are wrapper types that contain handles
                            return quote! {
                                // Get registry once before the loop
                                if let Some(registry) = self.get_registry() {
                                    // Migrate all object handles in the vector (treating them as wrappers)
                                    for wrapper in &mut self.#field_name {
                                        if let Ok(mut guard) = wrapper.handle().lock() {
                                            let type_name = guard.type_name();
                                            if reloaded_libs.contains(type_name) {
                                                // Serialize old object
                                                let data = guard.serialize_state()?;

                                                // Create new using existing infrastructure
                                                let mut new_obj = registry.call_constructor(
                                                    &format!("lib{}", type_name),
                                                    type_name,
                                                    ::hotline::RUSTC_COMMIT
                                                ).map_err(|e| e.to_string())?;

                                                // Restore state
                                                new_obj.deserialize_state(&data)?;
                                                new_obj.set_registry(registry);

                                                // Swap inside mutex
                                                *guard = new_obj;
                                            }
                                            // Recurse
                                            guard.migrate_children(reloaded_libs)?;
                                        }
                                    }
                                }
                            };
                        }
                    }
                }
                return quote! {};
            }
        }
    }

    // Handle direct object references
    quote! {
        {
            let handle = &self.#field_name;
            if let Ok(mut guard) = handle.lock() {
                let type_name = guard.type_name();
                if reloaded_libs.contains(type_name) {
                    // Serialize old object
                    let data = guard.serialize_state()?;

                    // Create new using existing infrastructure
                    if let Some(registry) = self.get_registry() {
                        let mut new_obj = registry.call_constructor(
                            &format!("lib{}", type_name),
                            type_name,
                            ::hotline::RUSTC_COMMIT
                        ).map_err(|e| e.to_string())?;

                        // Restore state
                        new_obj.deserialize_state(&data)?;
                        new_obj.set_registry(registry);

                        // Swap inside mutex
                        *guard = new_obj;
                    }
                }
                // Recurse
                guard.migrate_children(reloaded_libs)?;
            }
        }
    }
}

fn is_object_handle_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            // Check for Option<T> where T is an object
            if segment.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return is_object_type(inner_ty);
                    }
                }
            }
            // Check for Vec<T> where T is an object
            if segment.ident == "Vec" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return is_object_type(inner_ty);
                    }
                }
            }
            // Check if it's a direct object type
            return is_object_type(ty);
        }
    }
    false
}

fn is_option_object_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return is_object_type(inner_ty);
                    }
                }
            }
        }
    }
    false
}

fn is_option_object_wrapper_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                        return is_object_wrapper_type(inner_ty);
                    }
                }
            }
        }
    }
    false
}

fn is_object_wrapper_type(ty: &Type) -> bool {
    // Check if this is an object wrapper type (has a corresponding lib file)
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            let type_name = segment.ident.to_string();
            // Check if it's a known object by checking if a lib file exists
            let lib_path = crate::discovery::find_object_lib_file(&type_name);
            return lib_path.exists();
        }
    }
    false
}

fn is_object_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            let type_name = segment.ident.to_string();
            // Check if it's a known object type by checking if it starts with uppercase
            // and is not a standard type
            if type_name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                match type_name.as_str() {
                    // Standard library types
                    "String" | "Vec" | "Option" | "Result" | "Box" | "Arc" | "Mutex" |
                    "HashMap" | "HashSet" | "BTreeMap" | "BTreeSet" | "VecDeque" |
                    "Instant" | "Duration" | "SystemTime" | "PathBuf" | "Path" |
                    // Common trait objects
                    "EventHandler" |
                    // Known custom types that are not objects
                    "AtlasData" | "RenderCommand" | "AtlasFormat" | "SelectedObject" | "ResizeDir" => false,
                    _ => {
                        // Additional check: if it's a qualified path (e.g., std::time::Instant), it's not an object
                        if type_path.path.segments.len() > 1 {
                            false
                        } else {
                            true
                        }
                    }
                }
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    }
}

fn is_non_serializable_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        // Check full path for types like std::time::Instant
        let full_path = type_path.path.segments.iter().map(|s| s.ident.to_string()).collect::<Vec<_>>().join("::");

        match full_path.as_str() {
            "std::time::Instant" | "time::Instant" | "Instant" => true,
            _ => false,
        }
    } else {
        false
    }
}

fn contains_non_serializable_type(ty: &Type) -> bool {
    match ty {
        Type::Path(type_path) => {
            if is_non_serializable_type(ty) {
                return true;
            }

            if let Some(segment) = type_path.path.segments.last() {
                let type_name = segment.ident.to_string();

                // Check if it's a container type
                match type_name.as_str() {
                    "Vec" | "Option" | "Box" | "Arc" | "Mutex" | "VecDeque" => {
                        // Check if it contains non-serializable types
                        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                            for arg in &args.args {
                                if let syn::GenericArgument::Type(inner_ty) = arg {
                                    if contains_non_serializable_type(inner_ty) {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            false
        }
        _ => false,
    }
}

fn contains_trait_object(ty: &Type) -> bool {
    match ty {
        Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                let type_name = segment.ident.to_string();

                // Check if it's a container type
                match type_name.as_str() {
                    "Vec" | "Box" => {
                        // Check if it contains dyn Trait
                        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                            for arg in &args.args {
                                if let syn::GenericArgument::Type(inner_ty) = arg {
                                    if let Type::TraitObject(_) = inner_ty {
                                        return true;
                                    }
                                    if contains_trait_object(inner_ty) {
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            false
        }
        Type::TraitObject(_) => true,
        _ => false,
    }
}
