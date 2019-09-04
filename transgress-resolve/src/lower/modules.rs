use super::attributes::lower_metadata;
use crate::walker::WalkModuleCtx;
use syn::spanned::Spanned;
use transgress_api::idents::Ident;
use transgress_api::items::ModuleItem;

/// Lower a module declaration.
/// Does not handle internal attributes.
pub fn lower_module(ctx: &WalkModuleCtx, mod_: &syn::ItemMod) -> ModuleItem {
    let metadata = lower_metadata(ctx, &mod_.vis, &mod_.attrs, mod_.span());
    let name = Ident::from(&mod_.ident);
    ModuleItem { name, metadata }
}
