use super::{LowerError, ModuleCtx};
use crate::lower::attributes::lower_metadata;
use syn::spanned::Spanned;
use transgress_api::{idents::Ident, items::DeclarativeMacroItem, paths::Path, tokens::Tokens};

lazy_static::lazy_static! {
    pub static ref MACRO_RULES: Path = Path::fake("macro_rules");
    static ref MACRO_EXPORT: Path = Path::fake("macro_export");
}

/// Lower a `macro_rules!` declaration.
pub fn lower_macro_rules(
    ctx: &ModuleCtx,
    rules_: &syn::ItemMacro,
) -> Result<DeclarativeMacroItem, LowerError> {
    let mut metadata = lower_metadata(ctx, &syn::parse_quote!(pub), &rules_.attrs, rules_.span());
    let macro_export = metadata.extract_attribute(&*MACRO_EXPORT).is_some();

    if &Path::from(&rules_.mac.path) != &*MACRO_RULES {
        return Err(LowerError::NotAMacroDeclaration);
    }
    let name = Ident::from(
        rules_
            .ident
            .as_ref()
            .ok_or(LowerError::NotAMacroDeclaration)?,
    );
    let tokens = Tokens::from(&rules_.mac);

    Ok(DeclarativeMacroItem {
        macro_export,
        metadata,
        name,
        tokens,
    })
}
