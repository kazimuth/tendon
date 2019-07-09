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

/// An unresolved path.
#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct UnresolvedPath {
    /// The components of the path.
    pub path: Vec<Ident>,
    /// Whether the path starts with `::`
    pub is_absolute: bool,
}

/// A path resolved within an absolute crate.
#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct AbsolutePath {
    /// The path within the crate.
    pub path: Vec<Ident>,
    /// The containing crate.
    pub crate_: AbsoluteCrate,
}

/// A path resolved to a generic argument in the current context.
#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct GenericPath {
    /// The identifier of the generic.
    pub generic: Ident,
}

/// A crate, absolutely resolved within a crate graph.
/// Each AbsoluteCrate in a crate graph maps to a single crate.
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
