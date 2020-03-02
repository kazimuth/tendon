use crate::expressions::ConstExpr;
use crate::paths::Ident;
use crate::Map;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::fmt;
use std::hash::Hash;

mod types;

pub use types::*;
use tracing::Id;

/// Uniquely identifies an item (within a namespace).
///
/// Completely different from an `Ident`[ifier], which is just a textual identifier.
///
/// Items are identified by their name concatenated to the scope they were introduced in.
/// An item may have many bindings but it will only ever have one `Identity`.
///
/// TODO: this representation could be optimized.
#[derive(Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Serialize, Deserialize)]
pub struct Identity {
    /// The containing crate.
    pub crate_: CrateId,

    /// The path within the crate.
    pub path: Vec<Ident>,
}
impl Identity {
    // Create a new Identity
    pub fn new<P, I>(crate_: &CrateId, path: P) -> Self
    where
        P: IntoIterator<Item = I>,
        I: Into<Ident>,
    {
        Identity {
            crate_: crate_.clone(),
            path: path.into_iter().map(Into::into).collect(),
        }
    }

    /// Create a path at the root of a crate
    pub fn root(crate_: &CrateId) -> Self {
        Identity {
            crate_: crate_.clone(),
            path: vec![],
        }
    }

    /// Add another component to the path.
    pub fn join(&mut self, elem: impl Into<Ident>) -> &mut Self {
        let elem = elem.into();
        assert!(!elem.contains("::"));
        self.path.push(elem);
        self
    }

    /// Clone a referenced path and add a component.
    pub fn clone_join(&self, elem: impl Into<Ident>) -> Self {
        let mut c = self.clone();
        c.join(elem);
        c
    }

    pub fn join_seq<P, I>(&mut self, path: P) -> &mut Self
    where
        P: IntoIterator<Item = I>,
        I: Into<Ident>,
    {
        for p in path {
            self.join(p);
        }
        self
    }

    pub fn clone_join_seq<P, I>(&self, path: P) -> Self
    where
        P: IntoIterator<Item = I>,
        I: Into<Ident>,
    {
        let mut c = self.clone();
        c.join_seq(path);
        c
    }

    /// Get the parent of a relative path.
    pub fn parent(&self) -> Option<Self> {
        let mut result = self.clone();
        if let Some(_) = result.path.pop() {
            Some(result)
        } else {
            None
        }
    }
}

impl fmt::Debug for Identity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.crate_.fmt(f)?;
        for seg in &self.path {
            f.write_str("::")?;
            f.write_str(&seg)?;
        }
        Ok(())
    }
}

/// Uniquely identifies a lifetime.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct LifetimeId {
    id: Identity,
}
debug!(LifetimeId, "{:?}", id);
impl LifetimeId {
    pub fn new(id: Identity) -> LifetimeId {
        // don't allow construction w/o apostrophe.
        assert!(&id.path.iter().last().unwrap().is_lifetime());

        LifetimeId {
            id
        }
    }
    pub fn id(&self) -> &Identity {
        &self.id
    }
}

/// Uniquely identifies a symbol, with generic arguments.
#[derive(Serialize, Deserialize, Clone)]
pub struct SymbolId {
    pub id: Identity,
    pub params: GenericParams,
}
debug!(SymbolId, "{:?}{:?}", id, params);

/// Uniquely identifies a trait, with generic arguments.
#[derive(Serialize, Deserialize, Clone)]
pub struct TraitId {
    /// The path to the trait.
    pub id: Identity,
    /// The trait's generic arguments, if present.
    pub params: GenericParams,
    /// If the trait is prefixed with `?`
    pub is_maybe: bool,
}
impl fmt::Debug for TraitId {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        if self.is_maybe {
            write!(f, "?")?;
        }
        write!(f, "{:?}{:?}", self.id, self.params)
    }
}

/// A crate, absolutely resolved within a crate graph.
/// Each AbsoluteCrate in a crate graph maps to a single crate.
/// TODO: intern, def worth it for these
#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct CrateId {
    /// The name of the crate.
    pub name: SmolStr,
    /// The version of the crate.
    pub version: SmolStr,
}

impl CrateId {
    /// Create a new `AbsoluteCrate`.
    pub fn new(name: impl Into<SmolStr>, version: impl Into<SmolStr>) -> Self {
        CrateId {
            name: name.into(),
            version: version.into(),
        }
    }

    /// The identity of the root of the crate (e.g. the containing module
    pub fn root(&self) -> Identity {
        Identity {
            crate_: self.clone(),
            path: vec![],
        }
    }
}

impl fmt::Debug for CrateId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.name)?;
        f.write_str("[")?;
        f.write_str(&self.version)?;
        f.write_str("]")
    }
}


lazy_static! {
    pub static ref TEST_CRATE_A: CrateId = CrateId::new("test_crate_a", "0.0.0");
    pub static ref TEST_CRATE_B: CrateId = CrateId::new("test_crate_b", "0.0.0");
    pub static ref TEST_CRATE_C: CrateId = CrateId::new("test_crate_c", "0.0.0");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug() {
        assert_eq!(
            format!(
                "{:?}",
                Identity::new(
                    &CrateId::new("fake_crate", "0.1.0-alpha1"),
                    &["test", "Thing"]
                )
            ),
            "fake_crate[0.1.0-alpha1]::test::Thing"
        );
    }
}
