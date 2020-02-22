use super::attributes::lower_metadata;
use super::LowerError;
use crate::walker::LocationMetadata;
use syn::spanned::Spanned;
use tendon_api::idents::Ident;
use tendon_api::items::ModuleItem;

/// Lower a module declaration.
/// Does not handle internal attributes.
pub(crate) fn lower_module(
    loc: &LocationMetadata,
    mod_: &syn::ItemMod,
) -> Result<ModuleItem, LowerError> {
    let metadata = lower_metadata(loc, &mod_.vis, &mod_.attrs, mod_.span())?;
    let name = Ident::from(&mod_.ident);
    Ok(ModuleItem { name, metadata })
}
