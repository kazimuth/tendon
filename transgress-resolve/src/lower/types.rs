//! Lowering for referenced types.
use super::LowerError;
use transgress_api::{
    expressions::ConstExpr,
    idents::Ident,
    paths::Path,
    tokens::Tokens,
    traits::Trait,
    types::{
        ArrayType, BareFnType, ImplTraitType, Lifetime, NeverType, PathType, PointerType,
        QSelfType, ReferenceType, SliceType, TraitObjectType, TupleType, Type, GenericArgs
    },
};

/// Lower a syn type to a transgress-api type (with `Unresolved` paths, no resolution happens here.)
pub fn lower_type(type_: &syn::Type) -> Result<Type, LowerError> {
    match type_ {
        syn::Type::Path(path) => lower_type_path(path),
        syn::Type::Slice(slice) => Ok(Type::Slice(SliceType {
            type_: Box::new(lower_type(&slice.elem)?),
        })),
        syn::Type::Array(array) => Ok(Type::Array(ArrayType {
            type_: Box::new(lower_type(&array.elem)?),
            len: ConstExpr(Tokens::from(&array.len)),
        })),
        syn::Type::Ptr(pointer) => Ok(Type::Pointer(PointerType {
            type_: Box::new(lower_type(&pointer.elem)?),
            mut_: pointer.mutability.is_some(),
        })),
        syn::Type::Reference(reference) => Ok(Type::Reference(ReferenceType {
            type_: Box::new(lower_type(&reference.elem)?),
            mut_: reference.mutability.is_some(),
            lifetime: reference
                .lifetime
                .as_ref()
                .map(|lt| Lifetime(Ident::from(&lt.ident))),
        })),
        syn::Type::Never(_) => Ok(Type::Never(NeverType)),
        syn::Type::Tuple(tuple) => Ok(Type::Tuple(TupleType {
            types: tuple
                .elems
                .iter()
                .map(|type_| lower_type(type_))
                .collect::<Result<Vec<Type>, LowerError>>()?,
        })),
        syn::Type::TraitObject(trait_object) => {
            let (traits, lifetimes) = extract_bounds(&trait_object.bounds)?;
            Ok(Type::TraitObject(TraitObjectType {
                traits, lifetimes
            }))
        },
        syn::Type::ImplTrait(impl_trait) => {
            let (traits, lifetimes) = extract_bounds(&impl_trait.bounds)?;
            Ok(Type::ImplTrait(ImplTraitType {
                traits, lifetimes
            }))
        },
        syn::Type::BareFn(bare_fn) => {
            if bare_fn.inputs.is_empty() {
                Ok(Type::BareFn(BareFnType {
                    args: bare_fn
                        .inputs
                        .iter()
                        .map(|arg| lower_type(&arg.ty))
                        .collect::<Result<Vec<Type>, LowerError>>()?,
                    ret: Box::new(lower_return_type(&bare_fn.output)?),
                    varargs: bare_fn.variadic.is_some(),
                    unsafe_: bare_fn.unsafety.is_some(),
                }))
            } else {
                Err(LowerError::UnhandledType(Tokens::from(&bare_fn.inputs)))
            }
        }
        syn::Type::Paren(paren) => lower_type(&paren.elem),
        syn::Type::Group(group) => lower_type(&group.elem),
        other => Err(LowerError::UnhandledType(Tokens::from(&other))),
    }
}

/// Lower a TypePath. Big, so broken out into its own function.
fn lower_type_path(path: &syn::TypePath) -> Result<Type, LowerError> {
    if let Some(qself) = &path.qself {
        // <T as Q>::V

        let self_ = Box::new(lower_type(&qself.ty)?);

        if qself.position != path.path.segments.len() - 1 {
            return Err(LowerError::MalformedType(
                Tokens::from(path),
                "qself position not at end of path",
            ));
        }

        let mut inner_path = path.path.clone();
        let output_ = inner_path.segments.pop().expect("qself path too short");
        let output_ = Ident::from(&output_.value().ident);

        if inner_path.segments.len() == 0 {
            return Err(LowerError::MalformedType(
                Tokens::from(path),
                "qself without trait",
            ));
        }

        let (path, generics) = path_to_parts(&inner_path)?;

        Ok(Type::QSelf(QSelfType {
            self_,
            output_,
            trait_: Trait {path, generics, is_maybe: false}
        }))
    } else {
        let (path, generics) = path_to_parts(&path.path)?;
        Ok(Type::Path(PathType {
            path,
            generics
        }))
    }
}

/// Split a syn::Path to its constituent actual path and generic arguments.
fn path_to_parts(path: &syn::Path) -> Result<(Path, GenericArgs), LowerError> {
    // No QSelf
    // check for generics
    let mut syn_args = None;

    for seg in path.segments.iter() {
        if !syn_args.is_none() {
            // generics were present earlier in path!
            return Err(LowerError::UnexpectedGenericInPath(Tokens::from(path)));
        }
        if !seg.arguments.is_empty() {
            syn_args = Some(&seg.arguments);
        }
    }

    let mut args = GenericArgs {
        lifetimes: vec![],
        types: vec![],
        bindings: vec![],
        consts: vec![],
    };

    match syn_args {
        Some(syn::PathArguments::AngleBracketed(brangled)) => {
            for arg in brangled.args.iter() {
                match arg {
                    syn::GenericArgument::Lifetime(lt) => {
                        args.lifetimes.push(Lifetime(Ident::from(&lt.ident)))
                    }
                    syn::GenericArgument::Type(ty) => {
                        args.types.push(lower_type(ty)?)
                    }
                    syn::GenericArgument::Binding(binding) => args.bindings.push((
                        Ident::from(&binding.ident),
                        lower_type(&binding.ty)?,
                    )),
                    syn::GenericArgument::Const(expr) => {
                        args.consts.push(ConstExpr(Tokens::from(&expr)))
                    }
                    _ => {
                        return Err(LowerError::MalformedType(
                            Tokens::from(&path),
                            "forbidden generic type in path",
                        ))
                    }
                }
            }
        }
        Some(syn::PathArguments::Parenthesized(parened)) => {
            // Fn(X,Y) -> Z
            // is lowered to
            // Fn<(X, Y), Output=Z>
            args.types.push(Type::Tuple(TupleType {
                types: parened
                    .inputs
                    .iter()
                    .map(|ty| lower_type(ty))
                    .collect::<Result<Vec<Type>, LowerError>>()?,
            }));
            // TODO is it always `Output`?
            args.bindings.push((Ident::from("Output"), lower_return_type(&parened.output)?));
        }
        _ => (),
    }

    Ok((Path::from(path), args))
}

/// Lower a return type.
fn lower_return_type(ret: &syn::ReturnType) -> Result<Type, LowerError> {
    match ret {
        syn::ReturnType::Type(_, ret) => Ok(lower_type(&ret)?),
        syn::ReturnType::Default => {
            Ok(Type::Tuple(TupleType { types: vec![] }))
        }
    }
}

/// Convert a set of type bounds to a list of trait bounds + a list of lifetime bounds
fn extract_bounds(bounds: &syn::punctuated::Punctuated<syn::TypeParamBound, syn::token::Add>) -> Result<(Vec<Trait>, Vec<Lifetime>), LowerError> {
    let mut traits = Vec::new();
    let mut lifetimes = Vec::new();
    for bound in bounds.iter() {
        match bound {
            syn::TypeParamBound::Trait(trait_bound) => {
                if trait_bound.lifetimes.is_some() {
                    return Err(LowerError::NoHRTBsYet(Tokens::from(bound)));
                }
                let (path, generics) = path_to_parts(&trait_bound.path)?;
                let is_maybe = if let syn::TraitBoundModifier::Maybe(_) = trait_bound.modifier {
                    true
                } else {
                    false
                };
                traits.push(Trait {
                    path,
                    generics,
                    is_maybe
                })
            }
            syn::TypeParamBound::Lifetime(lt) => {
                lifetimes.push(Lifetime(Ident::from(&lt.ident)))
            }
        }

    }

    Ok((traits, lifetimes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use syn::parse_quote;

    fn lower(s: &str) -> Result<Type, Box<dyn Error>> {
        Ok(lower_type(&syn::parse_str(s)?)?)
    }

    #[test]
    fn simple() {
        //println!("{:#?}", syn::parse_str::<syn::TypePath>("<BANANA as OCELOT>::RHODODENDRON").unwrap());
        assert_match!(lower("!"), Ok(Type::Never(_)));
        assert_match!(lower("()"), Ok(Type::Tuple(TupleType {types})) => {
            assert_eq!(types.len(), 0);
        });
    }

    #[test]
    fn impl_dyn_trait() {
        assert_match!(lower("dyn Banana<'a, X> + Copy + ?Sized + 'b"), Ok(Type::TraitObject(TraitObjectType { traits, lifetimes })) => {
            assert_eq!(traits.len(), 3);
            assert_eq!(lifetimes.len(), 1);
            assert_eq!(traits[0].path, Path::fake("Banana"));;
            assert_eq!(traits[0].generics.lifetimes[0].0, Ident::from("a"));
            assert_match!(traits[0].generics.types[0], Type::Path(PathType { path, ..}) => {
                assert_eq!(path, &Path::fake("X"));
            });
            assert_eq!(traits[1].path, Path::fake("Copy"));
            assert_eq!(traits[2].path, Path::fake("Sized"));
            assert_eq!(traits[2].is_maybe, true);
            assert_eq!(lifetimes[0].0, Ident::from("b"));
        });
        assert_match!(lower("impl Banana<'a, X> + Copy + ?Sized + 'b"), Ok(Type::ImplTrait(ImplTraitType { traits, lifetimes })) => {
            assert_eq!(traits.len(), 3);
            assert_eq!(lifetimes.len(), 1);
            assert_eq!(traits[0].path, Path::fake("Banana"));;
            assert_eq!(traits[0].generics.lifetimes[0].0, Ident::from("a"));
            assert_match!(traits[0].generics.types[0], Type::Path(PathType { path, ..}) => {
                assert_eq!(path, &Path::fake("X"));
            });
            assert_eq!(traits[1].path, Path::fake("Copy"));
            assert_eq!(traits[2].path, Path::fake("Sized"));
            assert_eq!(traits[2].is_maybe, true);
            assert_eq!(lifetimes[0].0, Ident::from("b"));
        });
    }

    #[test]
    fn qself() {
        // TODO is this actually legal?
        assert_match!(lower("<P>::Q"), Err(..));

        assert_match!(lower("<P<F=(::M,)> as F<'a, Z, 2>>::W"), Ok(Type::QSelf(QSelfType {
            self_, trait_, output_
        })) => {
            assert_match!(**self_, Type::Path(PathType { path, generics }) => {
                assert_eq!(path, &Path::fake("P"));
                assert_eq!(generics.bindings[0].0, Ident::from("F"));
                assert_match!(generics.bindings[0].1, Type::Tuple(TupleType { types }) => {
                    assert_match!(types[0], Type::Path(PathType { path, .. }) => {
                        assert_eq!(path, &Path::fake("::M"));
                    });
                });
            });
        });
    }

    #[test]
    fn lower_path() {
        assert_match!(lower("::some::Thing<'a, 'b, A, B, C=D, 1>"), Ok(Type::Path(PathType { path, generics })) => {
            assert_eq!(path, &Path::fake("::some::Thing"));

            assert_eq!(generics.lifetimes.len(), 2);
            assert_eq!(generics.types.len(), 2);

            assert_eq!(generics.bindings.len(), 1);
            assert_eq!(generics.consts.len(), 1);
            assert_eq!(generics.lifetimes[0].0, Ident::from("a"));
            assert_eq!(generics.lifetimes[1].0, Ident::from("b"));
            assert_match!(generics.types[0], Type::Path(PathType { path, .. }) => {
                assert_eq!(path, &Path::fake("A"));
            });
            assert_match!(generics.types[1], Type::Path(PathType { path, .. }) => {
                assert_eq!(path, &Path::fake("B"));
            });
            assert_eq!(generics.bindings[0].0, Ident::from("C"));
            assert_match!(generics.bindings[0].1, Type::Path(PathType { path, .. }) => {
                assert_eq!(path, &Path::fake("D"));
            });
            assert_eq!(generics.consts[0].0, Tokens::new("1").unwrap());
        });
    }

    #[test]
    fn malformed_path() {
        assert_match!(path_to_parts(&parse_quote!(::bees<A, B>::dog<A, B>)), Err(LowerError::UnexpectedGenericInPath(..)));
    }
}
