//! Fast paths and identifiers, type currently implemented using the `SmolStr` crate.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use smol_str::SmolStr;
use std::fmt;
use std::ops::Deref;

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
    pub fn new<P, I>(rooted: bool, path: P) -> Self
    where
        P: IntoIterator<Item = I>,
        I: Into<Ident>,
    {
        UnresolvedPath {
            rooted,
            path: path.into_iter().map(Into::into).collect(),
        }
    }

    /// Make a fake path for testing.
    pub fn fake(s: &str) -> Self {
        UnresolvedPath::from(&syn::parse_str::<syn::Path>(s).expect("failed to parse fake path"))
    }

    pub fn join(self, component: impl Into<Ident>) -> UnresolvedPath {
        let UnresolvedPath { mut path, rooted } = self;
        path.push(component.into());
        UnresolvedPath { path, rooted }
    }

    /// Clone a referenced path and add a component.
    pub fn clone_join(&self, elem: impl Into<Ident>) -> Self {
        self.clone().join(elem)
    }

    pub fn join_seq<P, I>(mut self, path: P) -> Self
    where
        P: IntoIterator<Item = I>,
        I: Into<Ident>,
    {
        for p in path {
            self = self.join(p);
        }
        self
    }

    pub fn clone_join_seq<P, I>(&self, path: P) -> Self
    where
        P: IntoIterator<Item = I>,
        I: Into<Ident>,
    {
        self.clone().join_seq(path)
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

/// A rust identifier.
/// Represented using a small-string optimization.
///
/// May be an invalid identifier:
/// Raw identifiers are represented as `r#thing`.
/// Lifetimes are represented as `'thing`.
/// Anonymous scopes are represented as `{anon_123}`.
///
/// TODO: do raw lifetime identifiers exist??
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Ident(SmolStr);
impl Ident {
    /// Create a raw identifier. The ident will have "r#" added to the front if not present.
    pub fn raw(name: &str) -> Ident {
        if name.starts_with("r#") {
            Ident(name.into())
        } else {
            Ident(format!("r#{}", name).into())
        }
    }
    /// Create a lifetime identifier.
    pub fn lifetime(name: &str) -> Ident {
        if name.starts_with("'") {
            Ident(name.into())
        } else {
            Ident(format!("'{}", name).into())
        }
    }
    /// Check if an identifier is raw.
    pub fn is_raw(&self) -> bool {
        self.0.starts_with("r#")
    }
    /// Check if an identifier is a lifetime.
    pub fn is_lifetime(&self) -> bool {
        self.0.starts_with("'")
    }
}

impl Serialize for Ident {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s: &str = &self.0;
        s.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Ident {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Ident(<&str>::deserialize(deserializer)?.into()))
    }
}

impl From<&str> for Ident {
    fn from(s: &str) -> Self {
        Ident(s.into())
    }
}
impl From<String> for Ident {
    fn from(s: String) -> Self {
        Ident(s.into())
    }
}
impl From<&proc_macro2::Ident> for Ident {
    fn from(s: &proc_macro2::Ident) -> Ident {
        Ident(s.to_string().into())
    }
}
impl<T: Into<Ident> + Clone> From<&T> for Ident {
    fn from(s: &T) -> Ident {
        s.clone().into()
    }
}

impl Deref for Ident {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for Ident {
    fn borrow(&self) -> &str {
        &*self
    }
}

impl std::fmt::Debug for Ident {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn ident_serialize() {
        assert_eq!(
            serde_json::from_str::<Ident>(&serde_json::to_string(&Ident::from("test")).unwrap())
                .unwrap(),
            Ident::from("test")
        );
    }

    #[test]
    fn special_idents() {
        assert_eq!(Ident::raw("r#a"), Ident::raw("a"));
        assert_eq!(Ident::lifetime("'a"), Ident::lifetime("a"));
    }

    #[test]
    fn syn_paths() {
        let syn_path: syn::Path = parse_quote!(a::b::C);
        assert_eq!(
            UnresolvedPath::from(&syn_path),
            UnresolvedPath {
                path: vec!["a".into(), "b".into(), "C".into()],
                rooted: false
            }
        );
        let syn_path_2: syn::Path = parse_quote!(::a::b::C);
        assert_eq!(
            UnresolvedPath::from(&syn_path_2),
            UnresolvedPath {
                path: vec!["a".into(), "b".into(), "C".into()],
                rooted: true
            }
        );
        let syn_path_crate: syn::Path = parse_quote!(crate::z);
        assert_eq!(
            UnresolvedPath::from(&syn_path_crate),
            UnresolvedPath {
                path: vec!["crate".into(), "z".into()],
                rooted: false
            }
        );
    }

    #[test]
    fn debug() {
        assert_eq!(
            format!(
                "{:?}",
                UnresolvedPath {
                    path: vec!["test".into(), "Thing".into()],
                    rooted: true
                }
            ),
            "::test::Thing"
        );
    }
}
