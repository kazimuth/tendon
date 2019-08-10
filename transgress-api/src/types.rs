//! Data representing types.
use crate::generics::TypeBounds;
use crate::{expressions::ConstExpr, generics::Lifetime, idents::Ident, paths::Path};
use serde::{Deserialize, Serialize};
use std::fmt;

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
pub enum Type {
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
impl fmt::Debug for Type {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Type::Path(e) => fmt::Debug::fmt(e, f),
            Type::Array(e) => fmt::Debug::fmt(e, f),
            Type::Slice(e) => fmt::Debug::fmt(e, f),
            Type::Reference(e) => fmt::Debug::fmt(e, f),
            Type::Pointer(e) => fmt::Debug::fmt(e, f),
            Type::Tuple(e) => fmt::Debug::fmt(e, f),
            Type::Never(e) => fmt::Debug::fmt(e, f),
            Type::QSelf(e) => fmt::Debug::fmt(e, f),
            Type::BareFn(e) => fmt::Debug::fmt(e, f),
            Type::ImplTrait(e) => fmt::Debug::fmt(e, f),
            Type::TraitObject(e) => fmt::Debug::fmt(e, f),
        }
    }
}

/// A path, possibly with generic arguments `Type<T1, T2, Assoc=T3>`
#[derive(Clone, Serialize, Deserialize)]
pub struct PathType {
    /// The path to this type.
    pub path: Path,
    /// The applied generics.
    pub generics: GenericParams,
}
debug!(PathType, "{:?}{:?}", path, generics);

/// An array, `[i32; n]`.
#[derive(Clone, Serialize, Deserialize)]
pub struct ArrayType {
    pub type_: Box<Type>,
    pub len: ConstExpr,
}
debug!(ArrayType, "[{:?}; {:?}]", type_, len);

/// A slice, `[i32]`.
#[derive(Clone, Serialize, Deserialize)]
pub struct SliceType {
    pub type_: Box<Type>,
}
debug!(SliceType, "[{:?}]", type_);

/// An (optionally, mutable) reference.
#[derive(Clone, Serialize, Deserialize)]
pub struct ReferenceType {
    /// The referenced type.
    pub type_: Box<Type>,
    /// Whether the reference is mutable.
    pub mut_: bool,
    /// The lifetime of this reference, if present.
    pub lifetime: Option<Lifetime>,
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
    pub type_: Box<Type>,
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
    pub types: Vec<Type>,
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
    pub self_: Box<Type>,
    /// `as Trait`
    pub trait_: Trait,
    /// `::Output`
    pub output_: Ident,
}
debug!(QSelfType, "<{:?} as {:?}>::{:?}", self_, trait_, output_);

/// `fn(i32, String) -> usize`
#[derive(Clone, Serialize, Deserialize)]
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
    /// Bounds on this type.
    pub bounds: TypeBounds,
}
debug!(ImplTraitType, "(impl {:?})", bounds);

/// `dyn Trait`
#[derive(Clone, Serialize, Deserialize)]
pub struct TraitObjectType {
    /// Bounds on this type.
    pub bounds: TypeBounds,
}
debug!(TraitObjectType, "(dyn {:?})", bounds);

/// A reference to a trait (not a declaration).
#[derive(Clone, Serialize, Deserialize)]
pub struct Trait {
    /// The path to the trait.
    pub path: Path,
    /// The trait's generic arguments, if present.
    pub generics: GenericParams,
    /// If the trait is prefixed with `?`
    pub is_maybe: bool,
}
impl fmt::Debug for Trait {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        if self.is_maybe {
            write!(f, "?")?;
        }
        write!(f, "{:?}{:?}", self.path, self.generics)
    }
}

/// Generic arguments to a type or trait.
/// https://doc.rust-lang.org/reference/paths.html#paths-in-expressions
/// Doesn't include constraints. Those are defined at the declaration site.
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct GenericParams {
    /// Lifetime parameters to a type.
    pub lifetimes: Vec<Lifetime>,
    /// Type arguments to a type.
    pub types: Vec<Type>,
    /// Type bindings (e.g. `Output=T`)
    pub type_bindings: Vec<(Ident, Type)>,
    /// Const generic bindings.
    /// https://github.com/rust-lang/rfcs/blob/master/text/2000-const-generics.md
    /// Note: some of these may be parsed as types unfortunately, need to fix that later
    /// in the pipeline.
    pub consts: Vec<ConstExpr>,
}
impl fmt::Debug for GenericParams {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        if self.lifetimes.len() == 0
            && self.types.len() == 0
            && self.type_bindings.len() == 0
            && self.consts.len() == 0
        {
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
        for type_ in &self.types {
            if first {
                first = false;
            } else {
                write!(f, ", ")?;
            }
            write!(f, "{:?}", type_)?;
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
    use crate::tokens::Tokens;

    #[test]
    fn lifetime_debug() {
        assert_eq!(&format!("{:?}", Lifetime(Ident::from("test"))), "'test");
    }

    #[test]
    fn is_void() {
        assert!(TupleType { types: vec![] }.is_void());
        assert!(!TupleType {
            types: vec![Type::Never(NeverType)]
        }
        .is_void());
    }

    #[test]
    fn formatting() {
        let type_ = Type::Tuple(TupleType {
            types: vec![
                Type::Path(PathType {
                    path: Path::fake("test::Type"),
                    generics: Default::default(),
                }),
                Type::Pointer(PointerType {
                    mut_: true,
                    type_: Box::new(Type::Array(ArrayType {
                        type_: Box::new(Type::Never(NeverType)),
                        len: ConstExpr(Tokens::from(5)),
                    })),
                }),
                Type::Reference(ReferenceType {
                    lifetime: Some(Lifetime(Ident::from("a"))),
                    mut_: false,
                    type_: Box::new(Type::Slice(SliceType {
                        type_: Box::new(Type::Never(NeverType)),
                    })),
                }),
                Type::BareFn(BareFnType {
                    unsafe_: true,
                    varargs: true,
                    ret: Box::new(Type::TraitObject(TraitObjectType {
                        bounds: TypeBounds {
                            lifetimes: vec![Lifetime(Ident::from("a")), Lifetime(Ident::from("b"))],
                            traits: vec![Trait {
                                path: Path::fake("FakeTrait"),
                                generics: Default::default(),
                                is_maybe: true,
                            }],
                        },
                    })),
                    args: vec![Type::QSelf(QSelfType {
                        output_: Ident::from("Wow"),
                        self_: Box::new(Type::Never(NeverType)),
                        trait_: Trait {
                            path: Path::fake("FakeTrait"),
                            generics: Default::default(),
                            is_maybe: false,
                        },
                    })],
                }),
                Type::ImplTrait(ImplTraitType {
                    bounds: TypeBounds {
                        traits: vec![Trait {
                            path: Path::fake("Bees"),
                            is_maybe: false,
                            generics: GenericParams {
                                lifetimes: vec![Lifetime(Ident::from("a"))],
                                types: vec![],
                                type_bindings: vec![(
                                    Ident::from("B"),
                                    Type::Path(PathType {
                                        path: Path::fake("Honey"),
                                        generics: Default::default(),
                                    }),
                                )],
                                consts: vec![ConstExpr(Tokens::from(27u8))],
                            },
                        }],
                        lifetimes: vec![],
                    },
                }),
            ],
        });
        println!("{:?}", type_);
        assert_eq!(&format!("{:?}", type_),
                   "(~test::Type, *mut [!; 5i32], &'a [!], unsafe fn(<! as ~FakeTrait>::Wow, ...) -> (dyn 'a + 'b + ?~FakeTrait), (impl ~Bees<'a, B=~Honey; 27u8>))");
    }
}
