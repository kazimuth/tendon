use crate::ident::Ident;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::fmt;

/// A (possibly unresolved) path.
#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum Path {
    /// We haven't yet discovered what this resolves to.
    Unresolved(UnresolvedPath),
    /// This resolves to an absolute item.
    Absolute(AbsolutePath),
    /// This resolves to a nearby generic argument.
    Generic(GenericPath),
}
impl Path {
    /// Make a fake path for testing.
    pub fn fake(s: &str) -> Self {
        let syn_path = &syn::parse_str::<syn::Path>(s).expect("failed to parse fake path");
        Path::Unresolved(syn_path.into())
    }
    /// Make a non-absolute path from a single ident.
    pub fn ident(i: Ident) -> Self {
        Path::Unresolved(UnresolvedPath { path: vec![i], is_absolute: false})
    }
}
impl From<&syn::Path> for Path {
    fn from(p: &syn::Path) -> Self {
        Path::Unresolved(p.into())
    }
}

/// An unresolved path.
/// Note: segments of this path don't include arguments,
/// like in rust proper.
/// That's because paths in signatures can only have types
/// at the ends, e.g. there's no such thing as a T<X>::Y.
/// (there is such a thing as a <T<X> as Q>::Y but that's
/// handled at the type level, see the `types` module.)
#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct UnresolvedPath {
    /// The components of the path.
    pub path: Vec<Ident>,
    /// Whether the path starts with `::`
    pub is_absolute: bool,
}

impl From<&syn::Path> for UnresolvedPath {
    fn from(p: &syn::Path) -> Self {
        let is_absolute = p.leading_colon.is_some();
        // note: we strip path arguments, those need to be handled
        // outside of here
        let path = p.segments.iter().map(
            |seg| (&seg.ident).into()
        ).collect();
        UnresolvedPath {is_absolute, path}
    }
}

/// A path resolved within an absolute crate.
#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct AbsolutePath {
    /// The containing crate.
    pub crate_: AbsoluteCrate,
    /// The path within the crate.
    pub path: Vec<Ident>,
}

impl AbsolutePath {
    /// Add another component to the path.
    pub fn join(&self, elem: impl Into<Ident>) -> Self {
        let elem = elem.into();
        assert!(!elem.contains("::"));

        let crate_ = self.crate_.clone();
        let mut path = self.path.clone();
        path.push(elem.into());

        AbsolutePath { crate_, path }
    }
    /// The parent of this path.
    pub fn parent(&self) -> Self {
        debug_assert!(self.path.len() > 0, "no parent of crate root");
        let crate_ = self.crate_.clone();
        let path = self.path[0..self.path.len() - 1].iter().cloned().collect();
        AbsolutePath { crate_, path }
    }
}

/// A path resolved to a generic argument in the current context.
#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct GenericPath {
    /// The identifier of the generic.
    pub generic: Ident,
}

/// A crate, absolutely resolved within a crate graph.
/// Each AbsoluteCrate in a crate graph maps to a single crate.
/// TODO: intern, def worth it for these
#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct AbsoluteCrate {
    /// The name of the crate.
    pub name: SmolStr,
    /// The version of the crate.
    pub version: SmolStr,
}

impl fmt::Debug for AbsoluteCrate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.name)?;
        f.write_str("[")?;
        f.write_str(&self.version)?;
        f.write_str("]")
    }
}

impl fmt::Debug for AbsolutePath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.crate_.fmt(f)?;
        for seg in &self.path {
            f.write_str("::")?;
            f.write_str(&seg)?;
        }
        Ok(())
    }
}

impl fmt::Debug for UnresolvedPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("?")?;
        for (i, seg) in self.path.iter().enumerate() {
            if i > 0 || self.is_absolute {
                f.write_str("::")?;
            }
            f.write_str(&seg)?;
        }
        Ok(())
    }
}

impl fmt::Debug for GenericPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("<")?;
        self.generic.fmt(f)?;
        f.write_str(">")?;
        Ok(())
    }
}

impl fmt::Debug for Path {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Path::Absolute(path) => path.fmt(f),
            Path::Unresolved(path) => path.fmt(f),
            Path::Generic(path) => path.fmt(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug() {
        assert_eq!(
            format!(
                "{:?}",
                Path::Absolute(AbsolutePath {
                    path: vec!["test".into(), "Thing".into()],
                    crate_: AbsoluteCrate {
                        name: "fake_crate".into(),
                        version: "0.1.0-alpha1".into()
                    }
                })
            ),
            "fake_crate[0.1.0-alpha1]::test::Thing"
        );
        assert_eq!(
            format!(
                "{:?}",
                Path::Unresolved(UnresolvedPath {
                    path: vec!["test".into(), "Thing".into()],
                    is_absolute: true
                })
            ),
            "?::test::Thing"
        );
        assert_eq!(
            format!(
                "{:?}",
                Path::Generic(GenericPath {
                    generic: "T".into()
                })
            ),
            "<T>"
        );
    }
}
