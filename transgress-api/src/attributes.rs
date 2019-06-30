//! Data representing attributes.
use crate::Path;
use serde::{Deserialize, Serialize};

/// Extra attributes on a struct, unknown to transgress.
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct ExtraAttributes {
    /// Attributes in the `MetaItem` format.
    pub metas: Vec<MetaItem>,
    /// Attributes not in the `MetaItem` format.
    /// Note: strings do not include outer #[].
    pub weirds: Vec<String>,
}

/// The syntax used by most, but not all, attributes, and the `meta` fragment specifier.
/// Note that most built-in attributes are already handled for you; this is for the ones
/// Transgress doesn't know about.
#[derive(Clone, Serialize, Deserialize)]
pub enum MetaItem {
    /// A path attribute, e.g. #[thing]
    Path(Path),
    /// An equals attribute, e.g. #[thing = "bananas"]
    /// Note that the `literal` here can be parsed into a `proc_macro2::Literal`.
    Eq { target: Path, literal: String },
    /// An call attribute, e.g. #[thing(thinga, "bees", thingb = 3, thing4(2))]
    Call { target: Path, args: Vec<MetaItem> },
}

impl MetaItem {
    /// Get the path of this item.
    pub fn get_path(&self) -> &crate::Path {
        match self {
            MetaItem::Path(p) => p,
            MetaItem::Eq { target, .. } => target,
            MetaItem::Call { target, .. } => target,
        }
    }
}

/// Metadata for exported symbols (functions and statics).
#[derive(Clone, Serialize, Deserialize)]
pub struct SymbolMetadata {
    /// If this symbol has the #[no_mangle] attribute
    pub no_mangle: bool,
    /// The #[export_name] of this symbol, if present.
    pub export_name: Option<String>,
    /// The #[link_section] of this symbol, if present.
    pub link_section: Option<String>,
}

/// Deprecation metadata.
#[derive(Clone, Serialize, Deserialize)]
pub struct Deprecation {
    /// Version deprecated since, if present.
    /// TODO: format?
    pub since: Option<String>,
    /// Deprecation reason, if present.
    pub reason: Option<String>,
}
