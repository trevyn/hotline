use syn::{
    ItemImpl, ItemStruct, braced,
    parse::{Parse, ParseStream},
};

pub struct ObjectInput {
    pub struct_item: ItemStruct,
    pub impl_blocks: Vec<ItemImpl>,
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
