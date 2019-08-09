use crate::{paths::Path, types::GenericArgs};
use serde::{Deserialize, Serialize};

/// A reference to a trait (not a declaration).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trait {
    /// The path to the trait.
    pub path: Path,
    /// The trait's generic arguments, if present.
    pub generics: GenericArgs,
    /// If the trait is prefixed with `?`
    pub is_maybe: bool,
}

