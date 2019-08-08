//! Data representing types.
use crate::{expressions::ConstExpr, idents::Ident, paths::Path, traits::Trait};
use serde::{Deserialize, Serialize};
use std::fmt;

/*
#[derive(Clone, Debug, Serialize, Deserialize)]
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
    /// If this type implements `core::clone::Clone, Debug`.
    pub clone: bool,
    /// If this type implements `core::default::Default`.
    pub default: bool,
    /// If this type implements `core::fmt::Debug`.
    pub debug: bool,
    /// If this type implements `core::fmt::Display`.
    pub display: bool,
}
*/

/// A reference to a type.
///
/// This is distinct from the declaration of the referenced type. For instance, if you had:
///
/// ```no_build
/// struct S {}
/// ```
///
/// that would be a StructItem, not a type.
/// But then, if you referenced that struct:
///
/// ```no_build
/// fn q(s: S) {}
///         ^ this is a Type
/// ```
///
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Type {
    Path(PathType),
    Array(ArrayType),
    Slice(SliceType),
    Reference(ReferenceType),
    Pointer(PointerType),
    Tuple(TupleType),
    Never(NeverType),
    Reified(ReifiedType),
    QSelf(QSelfType),
    BareFn(BareFnType),
    ImplTrait(ImplTraitType),
    TraitObject(TraitObjectType),
}

/// A simple path, without generic arguments.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PathType(pub Path);

/// An array, `[i32; n]`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArrayType {
    pub type_: Box<Type>,
    pub len: ConstExpr,
}

/// A slice, `[i32]`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SliceType {
    pub type_: Box<Type>,
}

/// An (optionally, mutable) reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReferenceType {
    /// The referenced type.
    pub type_: Box<Type>,
    /// Whether the reference is mutable.
    pub mut_: bool,
    /// The lifetime of this reference, if present.
    pub lifetime: Option<Lifetime>,
}

/// An (optionally, mutable) pointer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PointerType {
    /// The pointed-to type.
    pub type_: Box<Type>,
    /// Whether the pointer is mutable or const.
    pub mut_: bool,
}

/// A tuple, `(i32, i8, String)`.
/// If there are 0 arguments, this is the void type, `()`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TupleType {
    pub types: Vec<Type>,
}
impl TupleType {
    /// If this tuple is void.
    pub fn is_void(&self) -> bool {
        self.types.len() == 0
    }
}

/// The never type, `!`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NeverType;

/// A type `<T as Trait>::Output`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QSelfType {
    /// `T`
    pub self_: Box<Type>,
    /// `as Trait`
    pub trait_: Box<Trait>,
    /// `::Output`
    pub output_: Ident,
}

/// A type with generic arguments `Type<T1, T2, Assoc=T3>`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReifiedType {
    pub type_: Path,
    pub args: GenericArgs,
}

/// `fn(i32, String) -> usize`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BareFnType {
    /// The arguments to the function pointer.
    pub args: Vec<Type>,
    /// The return type of the function pointer.
    pub ret: Box<Type>,
    /// If the function pointer takes varargs, `...`.
    /// See https://github.com/rust-lang/rfcs/blob/master/text/2137-variadic.md
    pub varargs: bool,
    /// If the function pointer is unsafe.
    pub unsafe_: bool,
}

/// `impl TraitA`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImplTraitType {
    /// All of the traits implemented by this type.
    pub traits: Vec<Trait>,
}

/// `dyn Trait`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraitObjectType {
    /// The main trait of the trait object.
    pub trait_: Trait,
    /// Extra traits (can only be auto traits).
    pub extras: Vec<Trait>,
}

/// A lifetime. Doesn't include apostrophe.
#[derive(Clone, Serialize, Deserialize)]
pub struct Lifetime(pub Ident);
impl fmt::Debug for Lifetime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "'{}", self.0)
    }
}

/// Generic arguments to a type or trait.
/// https://doc.rust-lang.org/reference/paths.html#paths-in-expressions
/// Doesn't include constraints.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenericArgs {
    /// Lifetime arguments to a type.
    pub lifetimes: Vec<Lifetime>,
    /// Type arguments to a type.
    pub types: Vec<Box<Type>>,
    /// Type bindings (e.g. `Output=T`)
    pub bindings: Vec<(Ident, Box<Type>)>,
    /// Const generic bindings.
    /// https://github.com/rust-lang/rfcs/blob/master/text/2000-const-generics.md
    /// Note: somne of these may be parsed as types unfortunately, need to fix that later
    /// in the pipeline.
    pub consts: Vec<ConstExpr>,
}
