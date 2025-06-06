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

        let mut type_defs = Vec::new();
        let mut struct_item = None;
        let mut impl_blocks = Vec::new();
        let mut found_main_struct = false;

        while !content.is_empty() {
            let item: Item = content.parse()?;
            match &item {
                Item::Struct(s) if !found_main_struct => {
                    // Check if this is the main struct (has impl blocks following)
                    let fork = content.fork();
                    if fork.peek(syn::Token![impl]) {
                        struct_item = Some(s.clone());
                        found_main_struct = true;
                    } else {
                        type_defs.push(item);
                    }
                }
                Item::Impl(i) if found_main_struct => {
                    impl_blocks.push(i.clone());
                }
                _ if !found_main_struct => {
                    type_defs.push(item);
                }
                _ => {
                    return Err(syn::Error::new_spanned(&item, "Unexpected item after impl blocks"));
                }
            }
        }

        let struct_item = struct_item
            .ok_or_else(|| syn::Error::new(content.span(), "Expected a main struct definition with impl blocks"))?;

        if impl_blocks.is_empty() {
            return Err(syn::Error::new(content.span(), "Expected at least one impl block after the struct"));
        }

        Ok(ObjectInput { type_defs, struct_item, impl_blocks })
    }
}
