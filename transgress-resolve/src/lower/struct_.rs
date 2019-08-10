/*use super::{LowerError, ModuleCtx};
use crate::lower::{
    attributes::lower_metadata,
    types::lower_type
};
use syn::spanned::Spanned;
use transgress_api::{
    items::{StructItem, StructField, StructKind, InherentImpl},
    idents::Ident
};

pub fn lower_struct(ctx: &ModuleCtx, struct_: &syn::ItemStruct) -> Result<StructItem, LowerError> {
    let metadata =
        super::attributes::lower_metadata(ctx, &struct_.vis, &struct_.attrs, struct_.span());
    if !struct_.generics.params.is_empty() {
        return Err(LowerError::NoGenericsYet(metadata.span));
    }

    let kind = match struct_.fields {
        syn::Fields::Named(..) => StructKind::Named,
        syn::Fields::Unnamed(..) => StructKind::Tuple,
        syn::Fields::Unit => StructKind::Unit,
    };

    let fields = struct_.fields.iter().enumerate().map(|(i, field)| {
        let metadata = lower_metadata(ctx, &field.vis, &field.attrs, field.span());
        let name = field.ident.as_ref().map(|ident| Ident::from(ident)).unwrap_or_else(|| Ident::from(&format!("{}", i)[..]));
        let type_ = lower_type(&field.ty)?;

        Ok(StructField { metadata, name, type_ })
    }).collect::<Result<Vec<_>, _>>()?;

    let inherent_impl = InherentImpl {};

    let type_metadata = panic!();

    Ok(StructItem {
        metadata,
        fields,
        kind,
        type_metadata,
        inherent_impl,
    })
}
*/
