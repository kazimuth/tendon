//! Extra data held in multiple diffferent items.

use crate::idents::Ident;
use crate::items::FunctionItem;
use crate::paths::Path;
use crate::tokens::Tokens;
use crate::types::Trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

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
    /// Other attributes on the item, unhandled by tendon.
    /// Note: this does *not* include cfg items! those are handled during parsing.
    pub extra_attributes: Vec<Attribute>,
    /// The span of this declaration.
    pub span: Span,
}
impl Metadata {
    /// Remove the first attribute with this path, if any.
    pub fn extract_attribute(&mut self, path: &Path) -> Option<Attribute> {
        let index = self
            .extra_attributes
            .iter()
            .position(|att| att.path() == path);
        index.map(|index| self.extra_attributes.remove(index))
    }

    pub fn fake() -> Metadata {
        // create a fake metadata. to be used only in testing.

        Metadata {
            visibility: Visibility::Pub,
            docs: None,
            must_use: None,
            deprecated: None,
            extra_attributes: vec![],
            span: Span::fake(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
/// A span in a source file.
pub struct Span {
    /// The source file, a path in the local filesystem.
    pub source_file: PathBuf,
    /// If we are expanding from a macro invocation, the invocation.
    /// If there are multiple levels, we keep only the top one.
    pub macro_invocation: Option<Arc<Span>>,
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
    pub fn new(
        macro_invocation: Option<Arc<Span>>,
        source_file: PathBuf,
        span: proc_macro2::Span,
    ) -> Self {
        // collapse a level of macro invocations. by induction, this will keep them at max 1 level deep.
        // TODO: retain this information? seems kinda pointless once macro expansions are thrown away
        let macro_invocation = if let Some(inv) = macro_invocation {
            if let Some(inv) = &inv.macro_invocation {
                debug_assert!(
                    inv.macro_invocation.is_none(),
                    "too many levels of span information..."
                );
                Some(inv.clone())
            } else {
                Some(inv)
            }
        } else {
            None
        };

        Span {
            source_file,
            macro_invocation,
            start_line: span.start().line as u32,
            start_column: span.start().column as u32,
            end_line: span.end().line as u32,
            end_column: span.end().column as u32,
        }
    }

    pub fn fake() -> Span {
        Span {
            source_file: "fake_file.rs".into(),
            macro_invocation: None,
            start_line: 0,
            start_column: 0,
            end_line: 0,
            end_column: 0,
        }
    }
}
impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        if let Some(span) = &self.macro_invocation {
            write!(
                f,
                "macro invocation at {}[{}:{}-{}:{}]",
                span.source_file.display(),
                span.start_line,
                span.start_column,
                span.end_line,
                span.end_column
            )
        } else {
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
}
impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        fmt::Debug::fmt(self, f)
    }
}

/// An attribute on an item.
///
/// Note that most built-in attributes are already handled for you; this is for the ones
/// tendon doesn't know about.
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
/// TODO: do we need more rules to handle wacky shadowing situations?
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
    /// The #[repr] of this type. `Rust` if no attribute is present.
    pub repr: Repr,
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
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Repr {
    /// `#[repr(Rust)]`
    Rust,
    /// `#[repr(C)]`
    C,
    /// `#[repr(transparent)]`
    Transparent,
    /// `#[repr(packed)]`
    Packed,
    /// `#[repr(C, i8)]`, etc.
    /// See https://github.com/rust-lang/rfcs/blob/master/text/2195-really-tagged-unions.md
    IntOuterTag(Ident),
    /// `#[repr(i8)]` or other reprs.
    Other(Ident),
}

impl Default for Repr {
    fn default() -> Self {
        Repr::Rust
    }
}

/// Get the metadata f
pub trait HasMetadata {
    fn metadata(&self) -> &Metadata;
}

#[macro_export]
macro_rules! impl_has_metadata {
    (struct $type:ident) => (
        impl $crate::attributes::HasMetadata for $type {
            fn metadata(&self) -> &Metadata {
                &self.metadata
            }
        }
    );
    (enum $type:ident { $($variant:ident (_), )*}) => (
        impl $crate::attributes::HasMetadata for $type {
            fn metadata(&self) -> &Metadata {
                match self {
                    $(
                        $type::$variant(data) => data.metadata(),
                    )+
                }
            }
        }
    );
}

use crate::items::*;

impl_has_metadata!(
    enum MacroItem {
        Declarative(_),
        Procedural(_),
        Derive(_),
        Attribute(_),
    }
);
impl_has_metadata!(
    enum SymbolItem {
        Const(_),
        Static(_),
        Function(_),
    }
);
impl_has_metadata!(
    enum TypeItem {
        Struct(_),
        Enum(_),
        Trait(_),
    }
);
impl_has_metadata!(struct DeclarativeMacroItem);
impl_has_metadata!(struct ProceduralMacroItem);
impl_has_metadata!(struct DeriveMacroItem);
impl_has_metadata!(struct AttributeMacroItem);
impl_has_metadata!(struct ConstItem);
impl_has_metadata!(struct StaticItem);
impl_has_metadata!(struct StructItem);
impl_has_metadata!(struct EnumItem);
impl_has_metadata!(struct TraitItem);
impl_has_metadata!(struct ModuleItem);
impl_has_metadata!(struct FunctionItem);

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn debug_span() {
        let span = super::Span {
            source_file: PathBuf::from("fake_file.rs"),
            macro_invocation: None,
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
            "#[test(arg1, arg2 = \"thing\", 3i32)]"
        );
        let attr = Attribute::Other {
            path: Path::fake("test2"),
            input: Tokens::from(quote!(= i am a test)),
        };
        assert_eq!(&format!("{:?}", attr), "#[test2 = i am a test]");
    }
}
