//! Data representing types.
use crate::{expr::ConstExpr, Ident, Path, Trait};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct TypeProperties {
    /// If this type implements `core::marker::Copy`.
    pub copy: bool,
    /// If this type implements `core::marker::Send`.
    pub send: bool,
    /// If this type implements `core::marker::Sync`.
    pub sync: bool,
    /// If this type implements `core::marker::Sized`.
    pub sized: bool,
    /// If this type implements `core::marker::Unpin`.
    pub unpin: bool,
    /// If this type implements `core::clone::Clone`.
    pub clone: bool,
    /// If this type implements `core::default::Default`.
    pub default: bool,
    /// If this type implements `core::fmt::Debug`.
    pub debug: bool,
    /// If this type implements `core::fmt::Display`.
    pub display: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Type {
    Array(ArrayType),
    Slice(SliceType),
    Ref(RefType),
    Ptr(PtrType),
    Tuple(TupleType),
    Never(NeverType),
    Reified(ReifiedType),
    QSelf(QSelfType),
}

/// An array, `[i32; n]`.
#[derive(Clone, Serialize, Deserialize)]
pub struct ArrayType {
    pub type_: Box<Type>,
    pub len: ConstExpr,
}

/// A slice, `[i32]`.
#[derive(Clone, Serialize, Deserialize)]
pub struct SliceType {
    pub type_: Box<Type>,
}

/// An (optionally, mutable) reference.
#[derive(Clone, Serialize, Deserialize)]
pub struct RefType {
    /// The referenced type.
    pub type_: Box<Type>,
    /// Whether the reference is mutable.
    pub mut_: bool,
}

/// An (optionally, mutable) pointer.
#[derive(Clone, Serialize, Deserialize)]
pub struct PtrType {
    /// The pointed-to type.
    pub type_: Box<Type>,
    /// Whether the pointer is mutable or const.
    pub mut_: bool,
}
/// A tuple, `(i32, i8, String)`.
#[derive(Clone, Serialize, Deserialize)]
pub struct TupleType {
    pub types: Vec<Type>,
}
/// The never type, `!`.
#[derive(Clone, Serialize, Deserialize)]
pub struct NeverType {}

/// A type `<T as Trait>::Output`
#[derive(Clone, Serialize, Deserialize)]
pub struct QSelfType {
    /// `T`
    pub self_: Box<Type>,
    /// `as Trait`
    pub trait_: Box<Trait>,
    /// `::Output`
    pub output_: Ident,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PathType {
    /// A path, without generic arguments.
    pub path: Path,
}

/// A type with generic arguments.
#[derive(Clone, Serialize, Deserialize)]
pub struct ReifiedType {
    pub type_: Path,
    pub args: GenericArgs,
}

/// A lifetime.
#[derive(Clone, Serialize, Deserialize)]
pub struct Lifetime(pub smol_str::SmolStr);

/// Generic arguments to a type or trait.
#[derive(Clone, Serialize, Deserialize)]
pub struct GenericArgs {
    pub lifetimes: Vec<Lifetime>,
}
