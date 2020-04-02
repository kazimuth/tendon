use std::path::PathBuf;

use crate::identities::CrateId;
use crate::paths::Ident;
use crate::Map;
use serde::{Deserialize, Serialize};

/// Metadata for a crate instantiation. There's one of these for every separate semver version for
/// every crate in the dependency tree.
///
/// Conceptually, this holds data derived from cargo metadata. Data that requires looking at the source
/// code (e.g. if a crate is no_std) is computed later.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrateData {
    /// Which crate this is.
    pub id: CrateId,
    /// The dependencies of this crate (note: renamed according to Cargo.toml, but NOT according to
    /// `extern crate ... as ...;` statements
    pub deps: Map<Ident, CrateId>,
    /// The *activated* features of this crate.
    pub features: Vec<String>,
    /// The path to the crate's `Cargo.toml`.
    pub manifest_path: PathBuf,
    /// The entry file into the crate.
    /// Note that this isn't always `crate_root/src/lib.rs`, some crates do other wacky stuff.
    pub entry: PathBuf,
    /// If this crate is a proc-macro crate.
    pub is_proc_macro: bool,
    /// The version of this crate.
    pub rust_edition: RustEdition,
}
impl CrateData {
    pub fn fake(id: CrateId) -> Self {
        CrateData {
            id,
            deps: Default::default(),
            features: Default::default(),
            manifest_path: Default::default(),
            entry: Default::default(),
            is_proc_macro: false,
            rust_edition: RustEdition::Rust2018,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RustEdition {
    Rust2015,
    Rust2018,
}
