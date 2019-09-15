use super::{Meta, MetaInner};
use crate::walker::{WalkModuleCtx};
use crate::lower::{LowerError};
use tendon_api::idents::Ident;
use tracing::warn;

lazy_static::lazy_static! {
    static ref ALL: Ident = "all".into();
    static ref ANY: Ident = "any".into();
    static ref NOT: Ident = "not".into();
    static ref FEATURE: Ident = "feature".into();
}

// TODO: other #[cfg] stuff; harvest options from build scripts somehow?
// https://doc.rust-lang.org/reference/conditional-compilation.html
// https://internals.rust-lang.org/t/all-the-rust-features/4322

/// Walk the input to a `cfg` attribute and return whether it is enabled or not.
pub fn interp_cfg(ctx: &WalkModuleCtx, meta: &Meta) -> Result<(), LowerError> {
    use LowerError::CfgdOut;
    match meta {
        Meta::Path(_) => bool_err(false),
        Meta::Assign { path, literal } => {
            let ident = path.get_ident().ok_or_else(|| {
                warn!("path in cfg: {:?}", path);
                CfgdOut
            })?;
            let target = literal.parse::<syn::LitStr>().map_err(|_| {
                warn!("non-str in cfg: {:?}", literal);
                CfgdOut
            })?;
            if ident == &*FEATURE {
                return bool_err(ctx.crate_data.features.contains(&target.value()));
            }
            bool_err(false)
        }
        Meta::Call { path, args } => {
            let ident = path.get_ident().ok_or_else(|| {
                warn!("path in cfg: {:?}", path);
                CfgdOut
            })?;

            let mut failed = false;

            let args = args.iter().filter_map(|inner| match inner {
                MetaInner::Meta(meta) => Some(meta),
                MetaInner::Literal(lit) => {
                    warn!("tokens in cfg: {:?}", lit);
                    failed = true;
                    None
                }
            }).collect::<Vec<_>>();

            if failed {
                return bool_err(false);
            }

            if ident == &*NOT {
                bool_err(!interp_cfg(ctx, args.get(0).ok_or(CfgdOut)?).is_ok())
            } else if ident == &*ALL || ident == &*ANY {
                let op = if ident == &*ALL {
                    (|a, b| a && b) as fn(bool, bool) -> bool
                } else {
                    (|a, b| a || b) as fn(bool, bool) -> bool
                };
                let mut current = if ident == &*ALL {
                    true
                } else {
                    false
                };
                for arg in args {
                    current = op(current, interp_cfg(ctx, arg).is_ok());
                }

                bool_err(current)
            } else {
                warn!("unknown cfg op: {}", ident);
                bool_err(false)
            }
        }
    }
}

fn bool_err(b: bool) -> Result<(), LowerError> {
    if b {
        Ok(())
    } else {
        Err(LowerError::CfgdOut)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{CrateData, RustEdition};
    use tendon_api::paths::AbsoluteCrate;
    use crate::lower::attributes::lower_meta;
    use tracing::info;

    #[test]
    fn cfgs() {
        let features = vec!["a".to_string(), "b".to_string()];

        let crate_data = CrateData {
            crate_: AbsoluteCrate::new("fake_crate", "0.0.0"),
            deps: Default::default(),
            manifest_path: Default::default(),
            entry: Default::default(),
            is_proc_macro: false,
            rust_edition: RustEdition::Rust2018,

            features
        };
        
        test_ctx!(mut ctx);

        ctx.crate_data = &crate_data;

        macro_rules! assert_meta {
            ($ctx:ident, #[cfg($($elem:tt)+)], $val:expr) => {
                let lowered = lower_meta(&syn::parse_quote!($($elem)+));
                assert_eq!(interp_cfg(&$ctx, &lowered).is_ok(), $val);
            }
        }

        info!(".");
        assert_meta!(ctx, #[cfg(feature = "a")], true);
        assert_meta!(ctx, #[cfg(feature = "b")], true);
        assert_meta!(ctx, #[cfg(feature = "c")], false);
        assert_meta!(ctx, #[cfg(feature = "bananas")], false);
        info!("n");
        assert_meta!(ctx, #[cfg(not(feature = "a"))], false);
        assert_meta!(ctx, #[cfg(not(feature = "c"))], true);
        info!("any");
        assert_meta!(ctx, #[cfg(any(feature = "a", feature = "bananas"))], true);
        info!("all");
        assert_meta!(ctx, #[cfg(all(feature = "a", feature = "bananas"))], false);
        assert_meta!(ctx, #[cfg(all(feature = "a", feature = "b"))], true);


    }
}