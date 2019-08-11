//! Extra data held in multiple diffferent items.

use crate::paths::Path;
use crate::tokens::Tokens;
use crate::types::Trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// Metadata available for all items, struct fields, etc.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Metadata {
    /// The visibility of this item.
    /// We can only bind fully `pub` items so we only track whether that's true.
    pub visibility: Visibility,
    /// Docs for this item.
    pub docs: Option<String>,
    /// If this item is must_use, the must_use reason.
    pub must_use: Option<String>,
    /// If this item is deprecated, the deprecation reason.
    pub deprecated: Option<Deprecation>,
    /// Other attributes on the item, unhandled by transgress-rs.
    /// Note: this does *not* include cfg items! those are handled during parsing.
    pub extra_attributes: Vec<Attribute>,
    /// The span of this declaration.
    pub span: Span,
}

#[derive(Clone, Serialize, Deserialize)]
/// A span in a source file.
pub struct Span {
    /// The source file, a path in the local filesystem.
    pub source_file: PathBuf,
    /// The starting line.
    pub start_line: u32,
    /// The starting column.
    pub start_column: u32,
    /// The ending line.
    pub end_line: u32,
    /// The ending column.
    pub end_column: u32,
}
impl Span {
    pub fn from_syn(source_file: PathBuf, span: proc_macro2::Span) -> Self {
        Span {
            source_file,
            start_line: span.start().line as u32,
            start_column: span.start().column as u32,
            end_line: span.end().line as u32,
            end_column: span.end().column as u32,
        }
    }
}
impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "{}[{}:{}-{}:{}]",
            self.source_file.display(),
            self.start_line,
            self.start_column,
            self.end_line,
            self.end_column
        )
    }
}
impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt::Debug::fmt(self, f)
    }
}

/// An attribute on an item.
///
/// Note that most built-in attributes are already handled for you; this is for the ones
/// Transgress doesn't know about.
#[derive(Clone, Serialize, Deserialize)]
pub enum Attribute {
    /// An attribute in the format of the
    /// [`meta` fragment specifier](https://doc.rust-lang.org/reference/attributes.html#meta-item-attribute-syntax).
    Meta(Meta),
    /// An attribute not in the `meta` format.
    Other { path: Path, input: Tokens },
}
impl Attribute {
    /// Get the root path of this attribute, whatever its form.
    pub fn path(&self) -> &Path {
        match self {
            Attribute::Meta(Meta::Path(path)) => path,
            Attribute::Meta(Meta::Assign { path, .. }) => path,
            Attribute::Meta(Meta::Call { path, .. }) => path,
            Attribute::Other { path, .. } => path,
        }
    }
    /// Get the assigned string, if this is an Assign with a string literal.
    pub fn get_assigned_string(&self) -> Option<String> {
        if let Attribute::Meta(Meta::Assign { literal, .. }) = self {
            if let Ok(lit_str) = literal.parse::<syn::LitStr>() {
                return Some(lit_str.value());
            }
        }
        None
    }
}
impl fmt::Debug for Attribute {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            Attribute::Meta(meta) => write!(f, "#[{:?}]", meta),
            Attribute::Other { path, input } => write!(f, "#[{:?} {:?}]", path, input),
        }
    }
}

/// The syntax used by most, but not all, attributes, and the
/// [`meta` fragment specifier](https://doc.rust-lang.org/reference/attributes.html#meta-item-attribute-syntax).
#[derive(Clone, Serialize, Deserialize)]
pub enum Meta {
    /// A path attribute, e.g. #[thing]
    Path(Path),
    /// An assignment attribute, e.g. #[thing = "bananas"]
    /// Note that the `literal` here can be parsed into a `proc_macro2::Literal`.
    Assign { path: Path, literal: Tokens },
    /// An call attribute, e.g. #[thing(thinga, "bees", thingb = 3, thing4(2))]
    Call { path: Path, args: Vec<MetaInner> },
}
impl fmt::Debug for Meta {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            Meta::Path(path) => write!(f, "{:?}", path),
            Meta::Assign { path, literal } => write!(f, "{:?} = {:?}", path, literal),
            Meta::Call { path, args } => {
                write!(f, "{:?}(", path)?;
                let mut first = true;
                for arg in args {
                    if first {
                        first = false;
                    } else {
                        write!(f, ", ")?;
                    }
                    write!(f, "{:?}", arg)?;
                }
                write!(f, ")")
            }
        }
    }
}

/// An argument in a meta list.
#[derive(Clone, Serialize, Deserialize)]
pub enum MetaInner {
    Meta(Meta),
    Literal(Tokens),
}
impl fmt::Debug for MetaInner {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            MetaInner::Meta(meta) => write!(f, "{:?}", meta),
            MetaInner::Literal(tokens) => write!(f, "{:?}", tokens),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
/// The visibility of an item.
pub enum Visibility {
    Pub,
    NonPub,
}

/// Metadata for exported symbols (functions, statics).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SymbolMetadata {
    /// If this symbol has the #[no_mangle] attribute
    pub no_mangle: bool,
    /// The #[export_name] of this symbol, if present.
    pub export_name: Option<String>,
    /// The #[link_section] of this symbol, if present.
    pub link_section: Option<String>,
}

/// Metadata for exported types (structs, enums, unions, ...)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TypeMetadata {
    /// All #[derives] present on this type.
    pub derives: Vec<Trait>,
}

/// Deprecation metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Deprecation {
    /// Version deprecated since, if present.
    /// TODO: format?
    pub since: Option<String>,
    /// Deprecation note, if present.
    pub note: Option<String>,
}

/// A struct representation.
#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn debug_span() {
        let span = super::Span {
            source_file: PathBuf::from("fake_file.rs"),
            start_line: 0,
            start_column: 1,
            end_line: 2,
            end_column: 3,
        };
        assert_eq!(&format!("{}", span), "fake_file.rs[0:1-2:3]")
    }

    #[test]
    fn debug_attr() {
        let attr = Attribute::Meta(Meta::Call {
            path: Path::fake("test"),
            args: vec![
                MetaInner::Meta(Meta::Path(Path::fake("arg1"))),
                MetaInner::Meta(Meta::Assign {
                    path: Path::fake("arg2"),
                    literal: Tokens::from("thing"),
                }),
                MetaInner::Literal(Tokens::from(3)),
            ],
        });
        assert_eq!(
            &format!("{:?}", attr),
            "#[~test(~arg1, ~arg2 = \"thing\", 3i32)]"
        );
        let attr = Attribute::Other {
            path: Path::fake("test2"),
            input: Tokens::from(quote!(= i am a test)),
        };
        assert_eq!(&format!("{:?}", attr), "#[~test2 = i am a test]");
    }
}
