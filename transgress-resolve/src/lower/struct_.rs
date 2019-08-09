/*
use super::{LowerError, ModuleCtx};
use crate::lower::attributes::lower_metadata;
use syn;
use syn::spanned::Spanned;
use transgress_api::items::StructItem;

pub fn lower_struct(ctx: &ModuleCtx, struct_: &syn::ItemStruct) -> Result<StructItem, LowerError> {
    let metadata =
        super::attributes::lower_metadata(ctx, &struct_.vis, &struct_.attrs, struct_.span());
    if !struct_.generics.params.is_empty() {
        return Err(LowerError::NoGenericsYet(metadata.span));
    }

    let fields = struct_.fields.iter().map(|field| {
        let metadata = lower_metadata(ctx, &field.vis, &field.attrs, field.span());
        0
    });
    panic!();
}
*/
