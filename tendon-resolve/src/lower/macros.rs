use super::LowerError;
use crate::lower::attributes::lower_metadata;
use crate::walker::LocationMetadata;
use syn::spanned::Spanned;
use tendon_api::{idents::Ident, items::DeclarativeMacroItem, paths::Path, tokens::Tokens};

lazy_static::lazy_static! {
    pub static ref MACRO_RULES: Path = Path::fake("macro_rules");
    static ref MACRO_EXPORT: Path = Path::fake("macro_export");
}

/// Lower a `macro_rules!` declaration.
pub(crate) fn lower_macro_rules(
    loc: &LocationMetadata,
    rules_: &syn::ItemMacro,
) -> Result<DeclarativeMacroItem, LowerError> {
    let mut metadata = lower_metadata(loc, &syn::parse_quote!(pub), &rules_.attrs, rules_.span())?;
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
    // note: store full declaration in tokens
    let tokens = Tokens::from(&rules_);

    Ok(DeclarativeMacroItem {
        macro_export,
        metadata,
        name,
        tokens,
    })
}
