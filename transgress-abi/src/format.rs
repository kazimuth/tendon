//! The transgress ABI serialization format.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
/// A valid Rust identifier.
pub struct Ident(String);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
/// Unique identifier for a type.
pub struct TypeIdent(pub Ident);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
/// Unique identifier for a type.
pub struct FunctionIdent(pub Ident);

/// Unique identifier for a trait.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TraitIdent(pub Ident);

/// Unique identifier for a lifetime.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LifetimeIdent(pub Ident);

/// The ABI for a single crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateAbi {
    pub doc: String,
    pub name: String,
    pub version: String,
    pub hash: String,
    pub compiler: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A single enum.
pub struct Enum {}

/// A single struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Struct {
    pub ident: TypeIdent,
    pub doc: String,
    pub generic: Generic,
    pub size: usize,
    pub align: usize,
    pub repr: StructRepr,
}

/// A struct representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StructRepr {
    Rust,
    C,
    Transparent,
}

/// A generic bound on a type / trait.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Generic {
    pub ident: TypeIdent,
    pub doc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lifetime {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trait {
    pub doc: String,
    pub types: Vec<Generic>,
}
