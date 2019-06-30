use crate::ident::Ident;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

/// A path.
/// TODO: resolution?: what guarantees do we provide about resolved paths?
/// - [multiple] reexports of inaccessible items? maybe create a "fake" module where they're accessible?
///   or just resolve every path in-ctx on use?
#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum Path {
    Unreso,
}

#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct UnresolvedPath {
    /// The components of the path.
    pub path: Vec<Ident>,
    /// Whether the path starts with `::`
    pub is_absolute: bool,
}

#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct ResolvedPath {
    pub path: Vec<Ident>,
    pub crate_: AbsoluteCrate,
}

#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct AbsoluteCrate {
    /// The name of the crate
    pub name: SmolStr,
    /// The version of the crate.
    pub version: SmolStr,
}

pub struct UnambiguousCrate {
    pub x: i32,
}
