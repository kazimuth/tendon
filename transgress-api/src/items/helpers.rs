//! Extra data held in multiple diffferent items.

use crate::Path;
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
    pub extra_attributes: Vec<Attribute>,
}




/// An attribute on an item.
///
/// Note that most built-in attributes are already handled for you, incl. derive a; this is for the ones
/// Transgress doesn't know about.
#[derive(Clone, Serialize, Deserialize)]
pub enum Attribute {
    MetaItem(MetaItem),
    /// An attribute not in the format understood by the `m` format.
    Other(String)
}

/// The syntax used by most, but not all, attributes, and the `meta` fragment specifier.
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

/// The visibility of an item.
pub enum Visibility {
    PubCrate,

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

/// A struct representation.
#[derive(Clone, Serialize, Deserialize)]
pub enum StructRepr {
    Rust,
    C,
    Transparent,
    Packed,
}
impl Default for StructRepr {
    fn default() -> Self {
        StructRepr::Rust
    }
}

/// An enum representation.
#[derive(Clone, Serialize, Deserialize)]
pub enum EnumRepr {
    /// Default.
    Rust,
    /// `#[repr(C)]`
    C,
    /// `#[repr(i8)]`, etc.
    Int(Int),
    /// `#[repr(C, i8)]`, etc.
    /// See https://github.com/rust-lang/rfcs/blob/master/text/2195-really-tagged-unions.md
    IntOuterTag(Int),
}
impl Default for EnumRepr {
    fn default() -> Self {
        EnumRepr::Rust
    }
}

/// An Int, used in an `EnumRepr`.
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum Int {
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
}
