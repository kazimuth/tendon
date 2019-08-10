use crate::{
    expressions::ConstExpr,
    idents::Ident,
    types::{Trait, Type},
};
/// Helpers for defining generics.
use serde::{Deserialize, Serialize};
use std::fmt;

/// A lifetime. Doesn't include apostrophe.
#[derive(Clone, Serialize, Deserialize)]
pub struct Lifetime(pub Ident);
impl fmt::Debug for Lifetime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "'{}", self.0)
    }
}

/// Generics embedded in a declaration.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Generics {
    /// Type parameters.
    pub type_params: Vec<TypeParameter>,
    /// Lifetime parameters.
    pub lifetime_params: Vec<Lifetime>,
    /// Constexpr parameters.
    pub const_params: Vec<ConstParameter>,
    /// Bounds on type parameters.
    pub type_bounds: Vec<(Type, TypeBounds)>,
    /// Bounds on lifetime parameters.
    pub lifetime_bounds: Vec<(Lifetime, LifetimeBounds)>,
}

/// A generic type parameter (at a declaration).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TypeParameter {
    /// The name of the type parameter.
    pub name: Ident,
    /// The default value of the type parameter, if present.
    pub default: Option<Type>,
}

/// A const type parameter (at a declaration).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConstParameter {
    /// The name of the parameter.
    pub name: Ident,
    /// The type of the parameter.
    pub type_: Type,
    /// The default value of the parameter, if present.
    pub value: Option<ConstExpr>,
}

/// Bounds on a generic argument, trait object, `impl Trait`, or existential type.
#[derive(Clone, Serialize, Deserialize)]
pub struct TypeBounds {
    /// The traits this type must satisfy.
    pub traits: Vec<Trait>,
    /// The lifetimes this type must satisfy.
    pub lifetimes: Vec<Lifetime>,
}
impl fmt::Debug for TypeBounds {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut first = true;
        for lifetime in &self.lifetimes {
            if first {
                first = false;
            } else {
                write!(f, " + ")?;
            }
            write!(f, "{:?}", lifetime)?;
        }
        for trait_ in &self.traits {
            if first {
                first = false;
            } else {
                write!(f, " + ")?;
            }
            write!(f, "{:?}", trait_)?;
        }
        Ok(())
    }
}

/// Bounds on a lifetime.
#[derive(Clone, Serialize, Deserialize)]
pub struct LifetimeBounds {
    /// The lifetimes this lifetime must satisfy.
    pub lifetimes: Vec<Lifetime>,
}

impl fmt::Debug for LifetimeBounds {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut first = true;
        for lifetime in &self.lifetimes {
            if first {
                first = false;
            } else {
                write!(f, " + ")?;
            }
            write!(f, "{:?}", lifetime)?;
        }
        Ok(())
    }
}
