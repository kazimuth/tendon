//! Macro expansion. Does not implement hygiene or operator precedence, since we only need to parse items.
//! Implemented as an interpreter on top of syn.
//!
//! This module is somewhat messy and could use a refactor.
//!
//! ## Expansion algorithm
//! Rust's macro expansion is actually quite subtle; it handles a lot of not-immediately-obvious edge cases.
//!

// from rust reference, https://doc.rust-lang.org/stable/reference/macros-by-example.html:

// > When forwarding a matched fragment to another macro-by-example, matchers in the second macro will
// > see an opaque AST of the fragment type. The second macro can't use literal tokens to match the fragments
// > in the matcher, only a fragment specifier of the same type. The ident, lifetime, and tt fragment types
// > are an exception, and can be matched by literal tokens.

//> The specific rules are:
//> expr and stmt may only be followed by one of: =>, ,, or ;.
//> pat may only be followed by one of: =>, ,, =, ,, if, or in.
//> path and ty may only be followed by one of: =>, ,, =, ,, ;, :, >, >>, [, {, as, where, or
//>     a macro variable of block fragment specifier.
//> vis may only be followed by one of: ,, an identifier other than a non-raw priv, any token
//>     that can begin a type, or a metavariable with a ident, ty, or path fragment specifier.
//> All other fragment specifiers have no restrictions.

use proc_macro2 as pm2;
use std::path::PathBuf;
use syn::spanned::Spanned;
use tendon_api::attributes::Span;
use tendon_api::idents::Ident;
use tendon_api::items::DeclarativeMacroItem;
use tendon_api::paths::AbsoluteCrate;
use tendon_api::tokens::Tokens;

mod ast;
mod consume;
mod transcribe;

/// Invoke a macro once.
pub fn apply_once(
    macro_: &DeclarativeMacroItem,
    tokens: pm2::TokenStream,
) -> syn::Result<pm2::TokenStream> {
    let rules = syn::parse2::<ast::MacroDef>(macro_.tokens.get_tokens())?;

    let mut stomach = consume::Stomach::new();

    for rule in &rules.rules {
        let result = stomach.consume(&tokens, &rule.matcher);
        if let Ok(()) = result {
            return transcribe::transcribe(&stomach.bindings, &rule.transcriber);
        } else if let Err(_) = result {
            stomach.reset();
        }
    }
    Err(syn::Error::new(
        tokens.span(),
        "failed to match any rule to macro input",
    ))
}

/*
// TODO reinstate
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::macros::lower_macro_rules;
    use quote::quote;

    #[test]
    fn full_macro() {
        spoor::init();
        test_ctx!(ctx);

        let rules: syn::ItemMacro = syn::parse_quote! { macro_rules! test_macro {
            ($($x:ident $y:ident),+) => ([$($x)+] [$($y)+]);
        }};

        let rules = lower_macro_rules(&ctx, &rules).unwrap();

        let input = quote!(a b, c d, e f);

        let output = apply_once(&rules, input).unwrap();

        assert_eq!(output.to_string(), quote!([a c e] [b d f]).to_string());
    }

    #[test]
    fn empty_macro() {
        spoor::init();
        test_ctx!(ctx);

        let rules: syn::ItemMacro = syn::parse_quote! { macro_rules! test_macro {
            () => (hooray);
        }};
        let rules = lower_macro_rules(&ctx, &rules).unwrap();

        let input = quote!();

        let output = apply_once(&rules, input).unwrap();

        assert_eq!(output.to_string(), quote!(hooray).to_string());
    }

    #[test]
    fn keyword_frag() {
        spoor::init();
        test_ctx!(ctx);

        let rules: syn::ItemMacro = syn::parse_quote! {
            macro_rules ! wacky_levels {
                ( $ ( $ name : ident ) ,+ | $ ( $ type : ty ) ,+ | $ ( $ expr : expr ) ,+ ) =>
                    { $ ( pub const $ name : $ type = $ expr ; ) + }
            }
        };
        let rules = lower_macro_rules(&ctx, &rules).unwrap();

        let input = quote!(hello, world | i32, i64 | 1, 2);

        let output = apply_once(&rules, input).unwrap();

        assert_eq!(
            output.to_string(),
            quote!(
                pub const hello: i32 = 1;
                pub const world: i64 = 2;
            )
            .to_string()
        );
    }

    #[test]
    fn multiple_rules() {
        spoor::init();
        test_ctx!(ctx);
        let rules: syn::ItemMacro = syn::parse_quote!(
            macro_rules! expands_to_item {
                ($(($x:ty)) 'f +) => {
                    ExpandedAlt {
                        thing: &'static std::option::Option<i32>,
                        stuff: ($($x),+)
                    }
                };
                () => {
                    Expanded {
                        thing: &'static std::option::Option<i32>
                    }
                }
            }
        );

        let rules = lower_macro_rules(&ctx, &rules).unwrap();

        let input = quote!();
        let output = apply_once(&rules, input).unwrap();
        assert_eq!(
            output.to_string(),
            quote!(
                Expanded {
                    thing: &'static std::option::Option<i32>
                }
            )
            .to_string()
        );

        let input = quote!((i32) 'f (i32) 'f (f64));
        let output = apply_once(&rules, input).unwrap();
        assert_eq!(
            output.to_string(),
            quote!(
                ExpandedAlt {
                    thing: &'static std::option::Option<i32>,
                    stuff: (i32, i32, f64)
                }
            )
            .to_string()
        );
    }

    #[test]
    fn simple_frag() {
        spoor::init();
        test_ctx!(ctx);

        let rules: syn::ItemMacro = syn::parse_quote! {
            macro_rules ! wacky_levels {
                ($i:ident) => ($i);
            }
        };
        let rules = lower_macro_rules(&ctx, &rules).unwrap();

        let input = quote!(hello);

        let output = apply_once(&rules, input).unwrap();

        assert_eq!(output.to_string(), quote!(hello).to_string());
    }

    #[test]
    fn rand() {
        // sample macro from `rand`.

        spoor::init();
        test_ctx!(ctx);

        let rules: syn::ItemMacro = syn::parse_quote! {
            macro_rules! impl_as_byte_slice {
                ($t:ty) => {
                    impl AsByteSliceMut for [$t] {
                        fn as_byte_slice_mut(&mut self) -> &mut [u8] {
                            if self.len() == 0 {
                                unsafe {
                                    // must not use null pointer
                                    slice::from_raw_parts_mut(0x1 as *mut u8, 0)
                                }
                            } else {
                                unsafe {
                                    slice::from_raw_parts_mut(&mut self[0]
                                        as *mut $t
                                        as *mut u8,
                                        self.len() * mem::size_of::<$t>()
                                    )
                                }
                            }
                        }

                        fn to_le(&mut self) {
                            for x in self {
                                *x = x.to_le();
                            }
                        }
                    }
                }
            }
        };
        let rules = lower_macro_rules(&ctx, &rules).unwrap();

        let input = quote!(i32);

        let output = apply_once(&rules, input).unwrap();

        assert_eq!(
            output.to_string(),
            quote!(
                impl AsByteSliceMut for [i32] {
                    fn as_byte_slice_mut(&mut self) -> &mut [u8] {
                        if self.len() == 0 {
                            unsafe {
                                // must not use null pointer
                                slice::from_raw_parts_mut(0x1 as *mut u8, 0)
                            }
                        } else {
                            unsafe {
                                slice::from_raw_parts_mut(&mut self[0]
                                    as *mut i32
                                    as *mut u8,
                                    self.len() * mem::size_of::<i32>()
                                )
                            }
                        }
                    }

                    fn to_le(&mut self) {
                        for x in self {
                            *x = x.to_le();
                        }
                    }
                }
            )
            .to_string()
        );
    }
}
*/
