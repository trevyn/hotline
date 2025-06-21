use syn::{
    Item, ItemImpl, ItemStruct, braced,
    parse::{Parse, ParseStream},
};

pub struct ObjectInput {
    pub type_defs: Vec<Item>,
    pub struct_item: ItemStruct,
    pub impl_blocks: Vec<ItemImpl>,
}

impl Parse for ObjectInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        braced!(content in input);

        let mut items = Vec::new();

        // First pass: collect all items
        while !content.is_empty() {
            let item: Item = content.parse()?;
            items.push(item);
        }

        // Second pass: find the main struct (the last public struct or the one with impl blocks)
        let mut type_defs = Vec::new();
        let mut struct_item = None;
        let mut impl_blocks = Vec::new();
        let mut found_main_struct = false;

        // Look for public structs first
        let mut public_struct_indices = Vec::new();
        for (i, item) in items.iter().enumerate() {
            if let Item::Struct(s) = item {
                if matches!(s.vis, syn::Visibility::Public(_)) {
                    public_struct_indices.push(i);
                }
            }
        }

        // Determine which struct is the main one
        let main_struct_idx = if public_struct_indices.len() == 1 {
            // If there's only one public struct, that's the main one
            Some(public_struct_indices[0])
        } else {
            // Otherwise, look for a struct followed by impl blocks for that struct
            let mut candidate_idx = None;
            for (i, item) in items.iter().enumerate() {
                if let Item::Struct(s) = item {
                    // Check if the next items are impl blocks for this struct
                    let struct_name = &s.ident;
                    if i + 1 < items.len() {
                        if let Item::Impl(impl_block) = &items[i + 1] {
                            if let Some((_, path, _)) = &impl_block.trait_ {
                                // This is a trait impl, not a direct impl
                                continue;
                            }
                            if let syn::Type::Path(type_path) = &*impl_block.self_ty {
                                if type_path.path.is_ident(struct_name) {
                                    candidate_idx = Some(i);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            candidate_idx
        };

        // Process items based on what we found
        if let Some(main_idx) = main_struct_idx {
            for (i, item) in items.into_iter().enumerate() {
                if i < main_idx {
                    // Everything before the main struct is a type definition
                    type_defs.push(item);
                } else if i == main_idx {
                    // This is the main struct
                    if let Item::Struct(s) = item {
                        struct_item = Some(s);
                        found_main_struct = true;
                    }
                } else if found_main_struct {
                    // Everything after the main struct should be impl blocks
                    match item {
                        Item::Impl(impl_block) => {
                            impl_blocks.push(impl_block);
                        }
                        _ => {
                            type_defs.push(item);
                        }
                    }
                }
            }
        } else {
            // Fallback: last struct is the main one
            let mut last_struct_idx = None;
            for (i, item) in items.iter().enumerate().rev() {
                if let Item::Struct(_) = item {
                    last_struct_idx = Some(i);
                    break;
                }
            }

            if let Some(idx) = last_struct_idx {
                for (i, item) in items.into_iter().enumerate() {
                    if i < idx {
                        type_defs.push(item);
                    } else if i == idx {
                        if let Item::Struct(s) = item {
                            struct_item = Some(s);
                            found_main_struct = true;
                        }
                    } else {
                        match item {
                            Item::Impl(impl_block) => impl_blocks.push(impl_block),
                            _ => type_defs.push(item),
                        }
                    }
                }
            }
        }

        let struct_item =
            struct_item.ok_or_else(|| syn::Error::new(content.span(), "Expected a main struct definition"))?;

        Ok(ObjectInput { type_defs, struct_item, impl_blocks })
    }
}
