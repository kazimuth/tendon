use crate::idents::Ident;
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
        Path::Unresolved(UnresolvedPath::fake(s))
    }
    /// Make a non-absolute path from a single ident.
    pub fn ident(i: Ident) -> Self {
        Path::Unresolved(UnresolvedPath {
            path: vec![i],
            rooted: false,
        })
    }
    /// Make a path to a generic.
    pub fn generic(generic: Ident) -> Self {
        Path::Generic(GenericPath { generic })
    }

    /// Get the path, assuming it's a single unresolved, non-absolute Ident.
    pub fn get_ident(&self) -> Option<&Ident> {
        if let Path::Unresolved(path) = self {
            path.get_ident()
        } else {
            None
        }
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
    pub rooted: bool,
}
impl UnresolvedPath {
    /// Make a fake path for testing.
    pub fn fake(s: &str) -> Self {
        UnresolvedPath::from(&syn::parse_str::<syn::Path>(s).expect("failed to parse fake path"))
    }

    pub fn join(self, component: Ident) -> UnresolvedPath {
        let UnresolvedPath {
            mut path,
            rooted
        } = self;
        path.push(component);
        UnresolvedPath { path, rooted }
    }

    /// Get the path, assuming it's a single unresolved, non-absolute Ident.
    pub fn get_ident(&self) -> Option<&Ident> {
        if self.path.len() == 1 && !self.rooted {
            Some(&self.path[0])
        } else {
            None
        }
    }
}
impl From<&syn::Path> for UnresolvedPath {
    fn from(p: &syn::Path) -> Self {
        let rooted = p.leading_colon.is_some();
        // note: we strip path arguments, those need to be handled
        // outside of here
        let path = p.segments.iter().map(|seg| (&seg.ident).into()).collect();
        UnresolvedPath { rooted, path }
    }
}
impl Into<Path> for UnresolvedPath {
    fn into(self) -> Path {
        Path::Unresolved(self)
    }
}

/// A path within a crate. Not a member of the `Path` enum.
/// TODO: this representation could be optimized.
#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize, PartialOrd, Ord)]
pub struct RelativePath(pub Vec<Ident>);

impl RelativePath {
    /// The relative path to the root of a crate
    pub fn root() -> Self {
        RelativePath(vec![])
    }

    /// Add another component to the relative path.
    pub fn join(mut self, elem: impl Into<Ident>) -> Self {
        let elem = elem.into();
        assert!(!elem.contains("::"));
        self.0.push(elem);
        self
    }

    /// Clone a referenced relative path and add a component.
    pub fn clone_join(&self, elem: impl Into<Ident>) -> Self {
        self.clone().join(elem)
    }

    /// Get the parent of a relative path.
    pub fn parent(&self) -> Option<Self> {
        let mut result = self.clone();
        if let Some(_) = result.0.pop() {
            Some(result)
        } else {
            None
        }
    }
}

impl<I, T> From<T> for RelativePath
where
    T: IntoIterator<Item = I>,
    I: Into<Ident>,
{
    fn from(t: T) -> Self {
        RelativePath(t.into_iter().map(Into::into).collect())
    }
}

/// A path resolved within an absolute crate.
/// TODO: this representation could be optimized.
#[derive(Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Serialize, Deserialize)]
pub struct AbsolutePath {
    /// The containing crate.
    pub crate_: AbsoluteCrate,

    /// The path within the crate.
    pub path: RelativePath,
}
impl AbsolutePath {
    // Create a new AbsolutePath
    pub fn new<P, I>(crate_: AbsoluteCrate, path: P) -> Self
    where
        P: IntoIterator<Item = I>,
        I: Into<Ident>,
    {
        AbsolutePath {
            crate_,
            path: RelativePath::from(path),
        }
    }
}
impl Into<Path> for AbsolutePath {
    fn into(self) -> Path {
        Path::Absolute(self)
    }
}

/// A path resolved to a generic argument in the current context.
#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct GenericPath {
    /// The identifier of the generic.
    pub generic: Ident,
}
impl Into<Path> for GenericPath {
    fn into(self) -> Path {
        Path::Generic(self)
    }
}

/// An identity of an item.
/// Items are identified by their name concatenated to the module they were introduced in.
/// An item may have many bindings but it will only ever have one `Identity`.
/// TODO: intern?
#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize, PartialOrd, Ord, Debug)]
pub struct Identity(pub AbsolutePath);

/// A crate, absolutely resolved within a crate graph.
/// Each AbsoluteCrate in a crate graph maps to a single crate.
/// TODO: intern, def worth it for these
#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize, PartialOrd, Ord)]
pub struct AbsoluteCrate {
    /// The name of the crate.
    pub name: SmolStr,
    /// The version of the crate.
    pub version: SmolStr,
}

impl AbsoluteCrate {
    /// Create a new `AbsoluteCrate`.
    pub fn new(name: impl Into<SmolStr>, version: impl Into<SmolStr>) -> Self {
        AbsoluteCrate {
            name: name.into(),
            version: version.into(),
        }
    }
}

impl fmt::Debug for AbsoluteCrate {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.name)?;
        f.write_str("[")?;
        f.write_str(&self.version)?;
        f.write_str("]")
    }
}

impl fmt::Debug for RelativePath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for seg in &self.0 {
            f.write_str("::")?;
            f.write_str(&seg)?;
        }
        Ok(())
    }
}

impl fmt::Debug for AbsolutePath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.crate_.fmt(f)?;
        self.path.fmt(f)?;
        Ok(())
    }
}

impl fmt::Debug for UnresolvedPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, seg) in self.path.iter().enumerate() {
            if i > 0 || self.rooted {
                f.write_str("::")?;
            }
            f.write_str(&seg)?;
        }
        Ok(())
    }
}

impl fmt::Debug for GenericPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("@")?;
        self.generic.fmt(f)?;
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
    use syn::parse_quote;

    #[test]
    fn syn_paths() {
        let syn_path: syn::Path = parse_quote!(a::b::C);
        assert_eq!(
            Path::from(&syn_path),
            Path::Unresolved(UnresolvedPath {
                path: vec!["a".into(), "b".into(), "C".into()],
                rooted: false
            })
        );
        let syn_path_2: syn::Path = parse_quote!(::a::b::C);
        assert_eq!(
            Path::from(&syn_path_2),
            Path::Unresolved(UnresolvedPath {
                path: vec!["a".into(), "b".into(), "C".into()],
                rooted: true
            })
        );
        let syn_path_crate: syn::Path = parse_quote!(crate::z);
        assert_eq!(
            Path::from(&syn_path_crate),
            Path::Unresolved(UnresolvedPath {
                path: vec!["crate".into(), "z".into()],
                rooted: false
            })
        );
    }

    #[test]
    fn debug() {
        assert_eq!(
            format!(
                "{:?}",
                Path::Absolute(AbsolutePath::new(
                    AbsoluteCrate::new("fake_crate", "0.1.0-alpha1"),
                    &["test", "Thing"]
                ))
            ),
            "fake_crate[0.1.0-alpha1]::test::Thing"
        );
        assert_eq!(
            format!(
                "{:?}",
                Path::Unresolved(UnresolvedPath {
                    path: vec!["test".into(), "Thing".into()],
                    rooted: true
                })
            ),
            "::test::Thing"
        );
        assert_eq!(
            format!(
                "{:?}",
                Path::Generic(GenericPath {
                    generic: "T".into()
                })
            ),
            "@T"
        );
    }
}
