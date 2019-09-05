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

// TODO: set macro recursion depth high
// TODO: multiple matchers per level?
// TODO: repro weird trace span-drops
// TODO: make ast Send + Serialize + store in DeclarativeMacroItem
// TODO: ensure sensible spans + error messages
// TODO: $crate

use proc_macro2 as pm2;
use syn::spanned::Spanned;
use tendon_api::items::DeclarativeMacroItem;

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
        if let Ok(()) = stomach.consume(&tokens, &rule.matcher) {
            return transcribe::transcribe(&stomach.bindings, &rule.transcriber);
        } else {
            stomach.reset();
        }
    }
    Err(syn::Error::new(
        tokens.span(),
        "failed to match any rule to macro input",
    ))
}
