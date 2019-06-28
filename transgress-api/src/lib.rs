//! Simple datastructures describing a rust program's interface: types, function signatures, consts, etc.
//! Produced and consumed by other `transgress` crates.
//!
//! Some inspiration taken from https://github.com/rust-lang/rls/tree/master/rls-data, although we represent
//! a significantly smaller subset of rust program metadata.

use serde::{Deserialize, Serialize};

mod ident;

pub use ident::Ident;

/// A single crate.
#[derive(Clone, Serialize, Deserialize)]
pub struct Crate {
    pub root_module: Module,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Module {
    pub item_metadata: ItemMetadata,
    pub submodules: Vec<Module>,
    pub enums: Vec<Enum>,
    pub structs: Vec<Struct>,
    pub free_functions: Vec<Function>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ItemMetadata {
    pub docs: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Struct {
    pub item_metadata: ItemMetadata,
    pub type_properties: TypeProperties,
    pub inherent_impl: InherentImpl,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Enum {
    pub item_metadata: ItemMetadata,
    pub type_properties: TypeProperties,
    pub inherent_impl: InherentImpl,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Function {
    pub ident: Ident,
    pub full_path: Path,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct InherentImpl {
    pub methods: Vec<Function>,
}

/// A path.
/// TODO: resolution?: what guarantees do we provide about resolved paths?
/// - [multiple] reexports of inaccessible items? maybe create a "fake" module where they're accessible?
#[derive(Clone, Serialize, Deserialize)]
pub struct Path {
    pub path: Vec<Ident>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TypeProperties {
    pub copy: bool,
    pub clone: bool,
    pub send: bool,
    pub sync: bool,
    pub sized: bool,
    pub unpin: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum StructRepr {
    Rust,
    C,
    Transparent,
    Packed,
}

#[cfg(test)]
mod tests {}
