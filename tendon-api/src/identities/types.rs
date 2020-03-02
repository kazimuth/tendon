//! Type identities. These are used to refer to types.

use crate::expressions::ConstExpr;
use crate::identities::{Identity, LifetimeId, TraitId};
use crate::paths::Ident;
use crate::Map;
use std::fmt;

use serde::{Deserialize, Serialize};

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
#[derive(Clone, Serialize, Deserialize)]
pub enum TypeId {
    Path(PathType),
    Array(ArrayType),
    Slice(SliceType),
    Reference(ReferenceType),
    Pointer(PointerType),
    Tuple(TupleType),
    Never(NeverType),
    QSelf(QSelfType),
    BareFn(BareFnType),
    ImplTrait(ImplTraitType),
    TraitObject(TraitObjectType),
}
impl TypeId {
    /// If this type is void.
    pub fn is_void(&self) -> bool {
        match self {
            TypeId::Tuple(t) => t.is_void(),
            _ => false,
        }
    }
}
impl fmt::Debug for TypeId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TypeId::Path(e) => fmt::Debug::fmt(e, f),
            TypeId::Array(e) => fmt::Debug::fmt(e, f),
            TypeId::Slice(e) => fmt::Debug::fmt(e, f),
            TypeId::Reference(e) => fmt::Debug::fmt(e, f),
            TypeId::Pointer(e) => fmt::Debug::fmt(e, f),
            TypeId::Tuple(e) => fmt::Debug::fmt(e, f),
            TypeId::Never(e) => fmt::Debug::fmt(e, f),
            TypeId::QSelf(e) => fmt::Debug::fmt(e, f),
            TypeId::BareFn(e) => fmt::Debug::fmt(e, f),
            TypeId::ImplTrait(e) => fmt::Debug::fmt(e, f),
            TypeId::TraitObject(e) => fmt::Debug::fmt(e, f),
        }
    }
}

/// A path, possibly with generic arguments `Type<T1, T2, Assoc=T3>`
#[derive(Clone, Serialize, Deserialize)]
pub struct PathType {
    /// The identity of the path's target.
    pub path: Identity,
    /// The applied generics.
    pub params: GenericParams,
}
debug!(PathType, "{:?}{:?}", path, params);

/// An array, `[i32; n]`.
#[derive(Clone, Serialize, Deserialize)]
pub struct ArrayType {
    pub type_: Box<TypeId>,
    pub len: ConstExpr,
}
debug!(ArrayType, "[{:?}; {:?}]", type_, len);

/// A slice, `[i32]`.
#[derive(Clone, Serialize, Deserialize)]
pub struct SliceType {
    pub type_: Box<TypeId>,
}
debug!(SliceType, "[{:?}]", type_);

/// An (optionally, mutable) reference.
#[derive(Clone, Serialize, Deserialize)]
pub struct ReferenceType {
    /// The referenced type.
    pub type_: Box<TypeId>,
    /// Whether the reference is mutable.
    pub mut_: bool,
    /// The lifetime of this reference, if present.
    pub lifetime: Option<LifetimeId>,
}
impl fmt::Debug for ReferenceType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut_ = if self.mut_ { "mut " } else { "" };
        if let Some(lt) = &self.lifetime {
            write!(f, "&{:?} {}{:?}", lt, mut_, self.type_)
        } else {
            write!(f, "&{}{:?}", mut_, self.type_)
        }
    }
}

/// An (optionally, mutable) pointer.
#[derive(Clone, Serialize, Deserialize)]
pub struct PointerType {
    /// The pointed-to type.
    pub type_: Box<TypeId>,
    /// Whether the pointer is mutable or const.
    pub mut_: bool,
}
impl fmt::Debug for PointerType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut_ = if self.mut_ { "mut " } else { "const " };
        write!(f, "*{}{:?}", mut_, self.type_)
    }
}

/// A tuple, `(i32, i8, String)`.
/// If there are 0 arguments, this is the void type, `()`.
#[derive(Clone, Serialize, Deserialize)]
pub struct TupleType {
    pub types: Vec<TypeId>,
}
impl TupleType {
    /// If this tuple is void.
    pub fn is_void(&self) -> bool {
        self.types.len() == 0
    }
}
impl fmt::Debug for TupleType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(")?;
        let mut first = true;
        for type_ in &self.types {
            if first {
                first = false;
            } else {
                write!(f, ", ")?;
            }
            write!(f, "{:?}", type_)?;
        }
        write!(f, ")")
    }
}

/// The never type, `!`.
#[derive(Clone, Serialize, Deserialize)]
pub struct NeverType;
debug!(NeverType, "!");

/// A type `<T as Trait>::Output`
#[derive(Clone, Serialize, Deserialize)]
pub struct QSelfType {
    /// `T`
    pub self_: Box<TypeId>,
    /// `as Trait`
    pub trait_: TraitId,
    /// `::Output`
    pub output_: Ident,
}
debug!(QSelfType, "<{:?} as {:?}>::{:?}", self_, trait_, output_);

/// `fn(i32, String) -> usize`
#[derive(Clone, Serialize, Deserialize)]
pub struct BareFnType {
    /// The arguments to the function pointer.
    pub args: Vec<TypeId>,
    /// The return type of the function pointer.
    pub ret: Box<TypeId>,
    /// If the function pointer takes varargs, `...`.
    /// See https://github.com/rust-lang/rfcs/blob/master/text/2137-variadic.md
    pub varargs: bool,
    /// If the function pointer is unsafe.
    pub unsafe_: bool,
}
impl fmt::Debug for BareFnType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.unsafe_ {
            write!(f, "unsafe ")?;
        }
        write!(f, "fn(")?;
        let mut first = true;
        for type_ in &self.args {
            if first {
                first = false;
            } else {
                write!(f, ", ")?;
            }
            write!(f, "{:?}", type_)?;
        }
        if self.varargs {
            write!(f, ", ...")?;
        }
        write!(f, ") -> {:?}", self.ret)
    }
}

/// `impl Trait`
#[derive(Clone, Serialize, Deserialize)]
pub struct ImplTraitType {
    /// Lifetime bounds on this type.
    pub lifetime_bounds: Vec<LifetimeId>,

    /// Trait bounds on this type.
    pub trait_bounds: Vec<TraitId>,
}
impl fmt::Debug for ImplTraitType {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "(impl ")?;
        let mut p = false;
        for b in &self.trait_bounds {
            if p {
                write!(f, " + ")?;
            } else {
                p = true;
            }
            write!(f, "{:?}", b)?;
        }
        for b in &self.lifetime_bounds {
            write!(f, " + {:?}", b)?;
        }
        write!(f, ")")
    }
}

/// `dyn Trait`
#[derive(Clone, Serialize, Deserialize)]
pub struct TraitObjectType {
    /// Bounds on this type.
    pub trait_bounds: Vec<TraitId>,
}
impl fmt::Debug for TraitObjectType {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "(dyn ")?;
        let mut p = false;
        for b in &self.trait_bounds {
            if p {
                write!(f, " + ")?;
            } else {
                p = true;
            }
            write!(f, "{:?}", b)?;
        }
        write!(f, ")")
    }
}

/// Generic arguments to a type or trait.
/// https://doc.rust-lang.org/reference/paths.html#paths-in-expressions
/// Doesn't include constraints. Those are defined at the declaration site.
/// Note: Default arguments are always present here.
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct GenericParams {
    /// Type bindings (e.g. `Output=T`).
    /// Maps the declaration parameters to their assignments.
    pub type_bindings: Map<Ident, TypeId>,

    /// Lifetime parameters to a type.
    /// Maps the declaration lifetimes to their assignments.
    pub lifetimes: Map<Ident, LifetimeId>,

    /// Const generic bindings.
    /// https://github.com/rust-lang/rfcs/blob/master/text/2000-const-generics.md
    /// Positional arguments are resolved before this structure is created.
    pub consts: Map<Ident, ConstExpr>,
}
impl GenericParams {
    pub fn empty() -> GenericParams {
        GenericParams {
            lifetimes: Default::default(),
            type_bindings: Default::default(),
            consts: Default::default(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.lifetimes.is_empty() && self.type_bindings.is_empty() && self.consts.is_empty()
    }
}

impl fmt::Debug for GenericParams {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        if self.lifetimes.len() == 0 && self.type_bindings.len() == 0 && self.consts.len() == 0 {
            return Ok(());
        }
        write!(f, "<")?;
        let mut first = true;
        for lt in &self.lifetimes {
            if first {
                first = false;
            } else {
                write!(f, ", ")?;
            }
            write!(f, "{:?}", lt)?;
        }
        for (name, type_) in &self.type_bindings {
            if first {
                first = false;
            } else {
                write!(f, ", ")?;
            }
            write!(f, "{:?}={:?}", name, type_)?;
        }
        if self.consts.len() > 0 {
            write!(f, "; ")?;
            let mut first = true;
            for const_ in &self.consts {
                if first {
                    first = false;
                } else {
                    write!(f, ", ")?;
                }
                write!(f, "{:?}", const_)?;
            }
        }
        write!(f, ">")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identities::TEST_CRATE_A;
    use crate::tokens::Tokens;

    #[test]
    fn lifetime_debug() {
        assert_eq!(
            &format!(
                "{:?}",
                LifetimeId::new(Identity::new(&*TEST_CRATE_A, &["Type", "'param"]))
            ),
            "test_crate_a[0.0.0]::Type::\'param"
        );
    }

    #[test]
    fn is_void() {
        assert!(TupleType { types: vec![] }.is_void());
        assert!(!TupleType {
            types: vec![TypeId::Never(NeverType)]
        }
        .is_void());
    }

    #[test]
    fn formatting() {
        let trait_ = TraitId {
            id: Identity::new(&*TEST_CRATE_A, &["TestTrait"]),
            params: Default::default(),
            is_maybe: false,
        };
        let mut type_bindings = Map::default();
        type_bindings.insert(
            Ident::from("A"),
            TypeId::Path(PathType {
                path: Identity::new(&*TEST_CRATE_A, &["other", "KindaType"]),
                params: Default::default(),
            }),
        );

        let type_ = TypeId::Tuple(TupleType {
            types: vec![
                TypeId::Path(PathType {
                    path: Identity::new(&*TEST_CRATE_A, &["test", "Type"]),
                    params: GenericParams {
                        type_bindings,
                        ..Default::default()
                    },
                }),
                TypeId::Pointer(PointerType {
                    mut_: true,
                    type_: Box::new(TypeId::Array(ArrayType {
                        type_: Box::new(TypeId::Never(NeverType)),
                        len: ConstExpr(Tokens::from(5)),
                    })),
                }),
                TypeId::Reference(ReferenceType {
                    lifetime: Some(LifetimeId::new(Identity::new(
                        &*TEST_CRATE_A,
                        &["Type", "'param"],
                    ))),
                    mut_: false,
                    type_: Box::new(TypeId::Slice(SliceType {
                        type_: Box::new(TypeId::Never(NeverType)),
                    })),
                }),
                TypeId::BareFn(BareFnType {
                    unsafe_: true,
                    varargs: true,
                    ret: Box::new(TypeId::TraitObject(TraitObjectType {
                        trait_bounds: vec![trait_.clone()],
                    })),
                    args: vec![TypeId::QSelf(QSelfType {
                        output_: Ident::from("Wow"),
                        self_: Box::new(TypeId::Never(NeverType)),
                        trait_,
                    })],
                }),
                TypeId::ImplTrait(ImplTraitType {
                    trait_bounds: vec![TraitId {
                        id: Identity::new(&*TEST_CRATE_A, &["TestTrait"]),
                        params: Default::default(),
                        is_maybe: false,
                    }],
                    lifetime_bounds: vec![],
                }),
            ],
        });
        println!("{:?}", type_);
        assert_eq!(&format!("{:?}", type_),
            "(test_crate_a[0.0.0]::test::Type<A=test_crate_a[0.0.0]::other::KindaType>, *mut [!; 5i32], &test_crate_a[0.0.0]::Type::\'param [!], unsafe fn(<! as test_crate_a[0.0.0]::TestTrait>::Wow, ...) -> (dyn test_crate_a[0.0.0]::TestTrait), (impl test_crate_a[0.0.0]::TestTrait))");
    }
}
