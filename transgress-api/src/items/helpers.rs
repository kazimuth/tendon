use crate::{attributes::ExtraAttributes, Ident, Path, Trait, Type};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct InherentImpl {}

/// Metadata available for all items.
#[derive(Clone, Serialize, Deserialize)]
pub struct ItemMetadata {
    /// Docs for this item.
    pub docs: String,
    /// If this item is must_use, the must_use reason.
    pub must_use: Option<String>,
    /// If this item is deprecated, the deprecation reason.
    pub deprecated: Option<Deprecation>,
    /// Other attributes on the item, unhandled by transgress-rs.
    pub extra_attributes: ExtraAttributes,
}

/// Metadata for exported symbols (functions, statics).
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
