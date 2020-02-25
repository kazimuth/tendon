use std::path::PathBuf;

use crate::identities::CrateId;
use crate::paths::Ident;
use crate::Map;
use serde::{Deserialize, Serialize};

/// Metadata for a crate instantiation. There's one of these for every separate semver version for
/// every crate in the dependency tree.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrateData {
    /// Which crate this is.
    pub crate_: CrateId,
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
    pub fn fake() -> Self {
        CrateData {
            crate_: CrateId::new("fake_crate", "0.0.0"),
            deps: Default::default(),
            features: Default::default(),
            manifest_path: "fake_crate/Cargo.toml".into(),
            entry: "fake_crate/src/lib.rs".into(),
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
