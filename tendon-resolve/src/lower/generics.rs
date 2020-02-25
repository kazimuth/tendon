//! Helpers for lowering generics.

use super::{types::lower_type, LowerError};
use crate::lower::types::lower_type_bounds;
use tendon_api::{
    expressions::ConstExpr,
    generics::{ConstParameter, Generics, Lifetime, LifetimeBounds, TypeParameter},
    paths::Ident,
    paths::Path,
    tokens::Tokens,
    types::Type,
};

/// Lower the generics on a declaration to a clean set of definitions and bounds.
pub fn lower_generics(generics: &syn::Generics) -> Result<Generics, LowerError> {
    let mut type_params = vec![];
    let mut lifetime_params = vec![];
    let mut const_params = vec![];
    let mut type_bounds = vec![];
    let mut lifetime_bounds = vec![];

    for param in &generics.params {
        match param {
            syn::GenericParam::Type(type_) => {
                let name = Ident::from(&type_.ident);
                type_params.push(TypeParameter {
                    name: name.clone(),
                    default: type_.default.as_ref().map(lower_type).transpose()?,
                });
                if !type_.bounds.is_empty() {
                    type_bounds.push((
                        Type::from(Path::generic(name)),
                        lower_type_bounds(&type_.bounds)?,
                    ))
                }
            }
            syn::GenericParam::Lifetime(def) => {
                let lifetime = lower_lifetime(&def.lifetime);
                lifetime_params.push(lifetime.clone());
                if !def.bounds.is_empty() {
                    lifetime_bounds.push((lifetime, lower_lifetime_bounds(&def.bounds)));
                }
            }
            syn::GenericParam::Const(const_) => const_params.push(ConstParameter {
                name: Ident::from(&const_.ident),
                type_: lower_type(&const_.ty)?,
                default: const_.default.as_ref().map(Tokens::from).map(ConstExpr),
            }),
        }
    }

    if let Some(where_clause) = &generics.where_clause {
        for predicate in where_clause.predicates.iter() {
            match predicate {
                syn::WherePredicate::Lifetime(predicate) => {
                    lifetime_bounds.push((
                        lower_lifetime(&predicate.lifetime),
                        lower_lifetime_bounds(&predicate.bounds),
                    ));
                }
                syn::WherePredicate::Type(predicate) => {
                    if predicate.lifetimes.is_some() {
                        return Err(LowerError::NoHRTBsYet(Tokens::from(predicate)));
                    }
                    type_bounds.push((
                        lower_type(&predicate.bounded_ty)?,
                        lower_type_bounds(&predicate.bounds)?,
                    ));
                }
                _ => return Err(LowerError::MalformedPredicate(Tokens::from(predicate))),
            }
        }
    }

    Ok(Generics {
        type_params,
        lifetime_params,
        const_params,
        type_bounds,
        lifetime_bounds,
    })
}

/// Lower a lifetime.
pub fn lower_lifetime(lifetime: &syn::Lifetime) -> Lifetime {
    Lifetime(Ident::from(&lifetime.ident))
}

pub fn lower_lifetime_bounds(
    bounds: &syn::punctuated::Punctuated<syn::Lifetime, syn::token::Add>,
) -> LifetimeBounds {
    LifetimeBounds {
        lifetimes: bounds.iter().map(lower_lifetime).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generics() {
        let type_: syn::ItemType = syn::parse_quote! {
            type T<'a: 'b, 'b, 'c, T, S: Copy, V=U, const WIDTH: usize = 3, const HEIGHT: usize>
                where 'b: 'c, T: Q, F: M<T>, S: Clone = !;
        };
        let generics = lower_generics(&type_.generics).unwrap();
        assert_eq!(generics.type_params.len(), 3);
        assert_eq!(generics.lifetime_params.len(), 3);
        assert_eq!(generics.const_params.len(), 2);
        assert_eq!(generics.lifetime_bounds.len(), 2);
        assert_eq!(generics.type_bounds.len(), 4);
        assert!(generics.type_params[2].default.is_some());
        assert!(generics.const_params[0].default.is_some());
    }
}
