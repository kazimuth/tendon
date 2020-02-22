//! Attribute lowering.

use super::LowerError;
use crate::walker::WalkModuleCtx;
use lazy_static::lazy_static;
use tendon_api::attributes::Repr;
use tendon_api::types::Trait;
use tendon_api::{
    attributes::{
        Attribute, Deprecation, Meta, MetaInner, Metadata, Span, SymbolMetadata, TypeMetadata,
        Visibility,
    },
    paths::Path,
    tokens::Tokens,
    types::GenericParams,
};
use tracing::{info_span, trace, warn};

mod interp_cfg;

lazy_static! {
    // the string used by `syn` for converting doc comments to attributes
    static ref DOCS: Path = Path::fake("doc");
    static ref MUST_USE: Path = Path::fake("must_use");
    static ref DEPRECATED: Path = Path::fake("deprecated");
    static ref SINCE: Path = Path::fake("since");
    static ref NOTE: Path = Path::fake("note");
    static ref DERIVE: Path = Path::fake("derive");
    static ref NO_MANGLE: Path = Path::fake("no_mangle");
    static ref EXPORT_NAME: Path = Path::fake("export_name");
    static ref LINK_SECTION: Path = Path::fake("link_section");
    static ref REPR: Path = Path::fake("repr");
    static ref REPR_RUST: Path = Path::fake("Rust");
    static ref REPR_C: Path = Path::fake("C");
    static ref REPR_TRANSPARENT: Path = Path::fake("transparent");
    static ref REPR_PACKED: Path = Path::fake("packed");
    static ref CFG: Path = Path::fake("cfg");
}

/// Lower a bunch of syn data structures to the generic `ItemMetadata`.
pub fn lower_metadata(
    ctx: &WalkModuleCtx,
    visibility: &syn::Visibility,
    attributes: &[syn::Attribute],
    span: proc_macro2::Span,
) -> Result<Metadata, LowerError> {
    let visibility = lower_visibility(visibility);
    let mut docs = None;
    let mut must_use = None;
    let mut deprecated = None;
    let mut extra_attributes = vec![];

    let span_ = Span::new(
        ctx.macro_invocation.clone(),
        ctx.source_file.to_path_buf(),
        span,
    );

    let _s = info_span!("lowering", span = &format!("{:?}", span_)[..]);
    let _s = _s.enter();

    for syn_attr in attributes {
        let attr = lower_attribute(syn_attr);
        if attr.path() == &*DOCS {
            docs = Some(
                if let Attribute::Meta(Meta::Assign { literal, .. }) = attr {
                    extract_string(&literal)
                } else {
                    trace!(
                        "unimplemented doc attribute {:?} [{:?}]",
                        attr,
                        Span::new(
                            ctx.macro_invocation.clone(),
                            ctx.source_file.clone(),
                            span.clone()
                        )
                    );
                    "".into()
                },
            );
        } else if attr.path() == &*MUST_USE {
            must_use = Some(
                if let Attribute::Meta(Meta::Assign { literal, .. }) = attr {
                    extract_string(&literal)
                } else {
                    warn!("malformed must_use attribute");
                    "".into()
                },
            );
        } else if attr.path() == &*DEPRECATED {
            deprecated = Some(if let Attribute::Meta(Meta::Call { args, .. }) = &attr {
                let mut since = None;
                let mut note = None;
                for arg in args {
                    if let MetaInner::Meta(Meta::Assign { path, literal }) = arg {
                        if path == &*SINCE {
                            since = Some(extract_string(&literal));
                        } else if path == &*NOTE {
                            note = Some(extract_string(&literal));
                        } else {
                            warn!("unexpected #[deprecated] arg: {:?}", path);
                        }
                    } else {
                        warn!("malformed #[deprecated]: {:?}", attr);
                    }
                }
                Deprecation { since, note }
            } else {
                warn!("malformed #[deprecated]: {:?}", attr);
                Deprecation {
                    since: None,
                    note: None,
                }
            })
        } else {
            extra_attributes.push(attr);
        }
    }

    let mut result = Metadata {
        visibility,
        docs,
        must_use,
        deprecated,
        extra_attributes,
        span: span_,
    };

    while let Some(cfg) = result.extract_attribute(&*CFG) {
        match cfg {
            Attribute::Meta(meta) => {
                interp_cfg::interp_cfg(ctx, &meta)?;
            }
            Attribute::Other { input, .. } => {
                warn!("bad cfg: {:?}", input);
                return Err(LowerError::CfgdOut);
            }
        }
    }

    Ok(result)
}

/// Lower a visibility. Assumes inherited visibilities aren't `pub`; you'll need to correct that
/// by hand.
pub fn lower_visibility(visibility: &syn::Visibility) -> Visibility {
    match visibility {
        syn::Visibility::Public(_) => Visibility::Pub,
        _ => Visibility::NonPub,
    }
}

/// Lower a syn attribute.
pub fn lower_attribute(attribute: &syn::Attribute) -> Attribute {
    if let Ok(meta) = attribute.parse_meta() {
        Attribute::Meta(lower_meta(&meta))
    } else {
        Attribute::Other {
            path: (&attribute.path).into(),
            input: Tokens::from(&attribute.tokens),
        }
    }
}

/// Lower a syn Meta to our Meta.
fn lower_meta(meta: &syn::Meta) -> Meta {
    // TODO: update this when syn merges the paths breaking change
    match meta {
        syn::Meta::Path(path) => Meta::Path(path.into()),
        syn::Meta::NameValue(syn::MetaNameValue { path, lit, .. }) => Meta::Assign {
            path: path.into(),
            literal: Tokens::from(lit),
        },
        syn::Meta::List(syn::MetaList { path, nested, .. }) => Meta::Call {
            path: path.into(),
            args: nested
                .iter()
                .map(|arg| match arg {
                    syn::NestedMeta::Meta(meta) => MetaInner::Meta(lower_meta(meta)),
                    syn::NestedMeta::Lit(lit) => MetaInner::Literal(Tokens::from(lit)),
                })
                .collect(),
        },
    }
}

/// TODO replace this w/ proper PM2 shim
fn extract_string(lit: &Tokens) -> String {
    if let Ok(lit) = syn::parse2::<syn::LitStr>(lit.get_tokens()) {
        lit.value()
    } else {
        warn!("failed to extract string from {:?}", lit);
        lit.get_tokens().to_string()
    }
}

/// Given a metadata, strip all the `extra_attributes` that go into a TypeMetadata.
pub fn extract_type_metadata(metadata: &mut Metadata) -> Result<TypeMetadata, LowerError> {
    // TODO: repr(align(n))

    let mut derives = vec![];
    let mut repr = Repr::Rust;
    metadata.extra_attributes.retain(|attribute| {
        if let Attribute::Meta(Meta::Call { path, args }) = attribute {
            if path == &*DERIVE {
                for arg in args {
                    if let MetaInner::Meta(Meta::Path(path)) = arg {
                        trace!("derive({:?})", path);
                        derives.push(Trait {
                            path: path.clone(),
                            params: GenericParams::default(),
                            is_maybe: false,
                        })
                    } else {
                        warn!("malformed #[derive]: {:?}", attribute)
                    }
                }
                return false; // remove this element
            } else if path == &*REPR {
                if args.len() == 1 {
                    if let MetaInner::Meta(Meta::Path(path)) = &args[0] {
                        if path == &*REPR_RUST {
                            // no change
                        } else if path == &*REPR_C {
                            repr = Repr::C;
                        } else if path == &*REPR_TRANSPARENT {
                            repr = Repr::Transparent;
                        } else if path == &*REPR_PACKED {
                            repr = Repr::Packed;
                        } else if let Some(ident) = path.get_ident() {
                            repr = Repr::Other(ident.clone());
                        } else {
                            warn!("malformed #[repr]: {:?}", attribute);
                        }
                    }
                } else if args.len() == 2 {
                    if let MetaInner::Meta(Meta::Path(path)) = &args[0] {
                        if path == &*REPR_C {
                            if let MetaInner::Meta(Meta::Path(path)) = &args[1] {
                                if let Some(ident) = path.get_ident() {
                                    repr = Repr::IntOuterTag(ident.clone());
                                    return false;
                                }
                            }
                        }
                    }
                    warn!("malformed #[repr]: {:?}", attribute)
                } else {
                    warn!("malformed #[repr]: {:?}", attribute)
                }
                return false;
            }
        }
        true
    });
    Ok(TypeMetadata { derives, repr })
}

/// Given a metadata, strip all the `extra_attributes` that go into a TypeMetadata.
pub fn extract_symbol_metadata(metadata: &mut Metadata) -> Result<SymbolMetadata, LowerError> {
    let mut no_mangle = false;
    let mut export_name = None;
    let mut link_section = None;
    metadata.extra_attributes.retain(|attribute| {
        if attribute.path() == &*NO_MANGLE {
            no_mangle = true;
            return false;
        } else if attribute.path() == &*EXPORT_NAME {
            if let Some(name) = attribute.get_assigned_string() {
                export_name = Some(name);
                return false;
            }
            warn!("malformed #[export_name] attribute: {:?}", attribute);
        } else if attribute.path() == &*LINK_SECTION {
            if let Some(section) = attribute.get_assigned_string() {
                link_section = Some(section);
                return false;
            }
            warn!("malformed #[link_section] attribute: {:?}", attribute);
        }
        true
    });
    Ok(SymbolMetadata {
        no_mangle,
        export_name,
        link_section,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;
    use syn::{parse_quote, spanned::Spanned};
    use tendon_api::attributes::Deprecation;

    #[test]
    fn metadata_lowering() {
        test_ctx!(ctx);
        let all = lower_metadata(
            &ctx,
            &parse_quote!(pub),
            &[
                parse_quote!(
                    /// this is an item that exists
                ),
                parse_quote!(#[must_use = "use me"]),
                parse_quote!(#[deprecated(since = "0.2.0", note = "don't use me")]),
                parse_quote!(#[other_attribute]),
                parse_quote!(#[other_attribute_meta(thing = "baz")]),
                parse_quote!(#[other_attribute_weird 2 + 2 / 3 - 4]),
            ],
            quote!(_).span(),
        )
        .unwrap();
        assert_match!(all, Metadata {
            visibility: Visibility::Pub,
            docs: Some(docs),
            must_use: Some(must_use),
            deprecated: Some(Deprecation { note: Some(note), since: Some(since) }),
            extra_attributes,
            ..
        } => {
            assert_eq!(docs, " this is an item that exists");
            assert_eq!(must_use, "use me");
            assert_eq!(since, "0.2.0");
            assert_eq!(note, "don't use me");

            assert_match!(extra_attributes[0], Attribute::Meta(Meta::Path(path)) => {
                assert_eq!(path, &Path::fake("other_attribute"))
            });

            assert_match!(extra_attributes[1], Attribute::Meta(Meta::Call {
                path, args
            }) => {
                assert_eq!(path, &Path::fake("other_attribute_meta"));
                assert_match!(args[0], MetaInner::Meta(Meta::Assign { path, literal }) => {
                    assert_eq!(path, &Path::fake("thing"));
                    assert_eq!(literal.get_tokens().to_string(), quote!("baz").to_string());
                });
            });

            assert_match!(extra_attributes[2], Attribute::Other{
                path, input
            } => {
                assert_eq!(path, &Path::fake("other_attribute_weird"));
                assert_eq!(input.to_string(), quote!(2 + 2 / 3 - 4).to_string());
            });
        });

        // shouldn't panic
        let funky = lower_metadata(
            &ctx,
            &parse_quote!(pub(crate)),
            &[
                parse_quote!(#[docs(bees = "superior")]),
                parse_quote!(#[must_use(dogs = "incredible")]),
                parse_quote!(#[deprecated = "nope"]),
                parse_quote!(#[deprecated(flim_flam = "funsy parlor")]),
            ],
            quote!(_).span(),
        )
        .unwrap();

        assert_eq!(funky.visibility, Visibility::NonPub);
    }
}
