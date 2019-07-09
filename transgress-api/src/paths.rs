use crate::ident::Ident;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

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
    pub path: Vec<Ident>,
    pub crate_: AbsoluteCrate,
}

/// A path resolved to a generic argument in the current context.
#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct GenericPath {
    /// The identifier of the generic.
    pub generic: SmolStr,
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
