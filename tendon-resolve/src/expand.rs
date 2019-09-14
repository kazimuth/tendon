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
use quote::quote;
use syn::spanned::Spanned;
use tendon_api::attributes::Span;
use tendon_api::idents::Ident;
use tendon_api::items::DeclarativeMacroItem;
use tendon_api::paths::{AbsoluteCrate, AbsolutePath, UnresolvedPath};
use tendon_api::tokens::Tokens;
use tracing::info;
use std::path::PathBuf;

mod ast;
mod consume;
mod transcribe;

/// Invoke a macro once.
pub fn apply_once(
    macro_: &DeclarativeMacroItem,
    tokens: pm2::TokenStream,
) -> syn::Result<pm2::TokenStream> {
    info!("parse macro: {:?}", macro_.tokens);
    let rules = syn::parse2::<ast::MacroDef>(macro_.tokens.get_tokens())?;

    let mut stomach = consume::Stomach::new();

    info!("apply rules");
    for rule in &rules.rules {
        if let Ok(()) = stomach.consume(&tokens, &rule.matcher) {
            info!("success, transcribing");
            return transcribe::transcribe(&stomach.bindings, &rule.transcriber);
        } else {
            info!("failed, next");
            stomach.reset();
        }
    }
    Err(syn::Error::new(
        tokens.span(),
        "failed to match any rule to macro input",
    ))
}

/// A module with macros unexpanded.
/// We throw all macro-related stuff here when we're walking freshly-parsed modules.
/// It's not possible to eagerly expand macros because they rely on name resolution to work, and we
/// can't do name resolution (afaict) until after we've lowered most modules already.
/// This is ordered because order affects macro name resolution.
#[derive(Debug)]
pub struct UnexpandedModule {
    items: Vec<UnexpandedItem>,
    pub source_file: PathBuf
}
impl UnexpandedModule {
    /// Create an empty unexpanded module.
    pub fn new(source_file: PathBuf) -> Self {
        UnexpandedModule {
            items: vec![],
            source_file
        }
    }
}

#[derive(Debug)]
/// An item that needs macro expansion.
/// TODO: do we need to store imports here as well?
pub enum UnexpandedItem {
    /// A macro invocation in item position. Note: the macro in question could be `macro_rules!`.
    MacroInvocation(Span, Tokens),
    /// Some item that contains a macro in type position.
    TypeMacro(Span, Tokens),
    /// Something with an attribute macro applied.
    AttributeMacro(Span, Tokens),
    /// Something with a derive macro applied.
    /// Note: the item itself should already be stored in the main `Db`, and doesn't need to be
    /// re-added.
    DeriveMacro(Span, Tokens),
    /// A sub module that has yet to be expanded.
    UnexpandedModule { span: Span, name: Ident, macro_use: bool },
    /// An import with #[macro_use].
    MacroUse(Span, AbsoluteCrate),
}
impl UnexpandedItem {
    pub fn span(&self) -> &Span {
        match self {
            UnexpandedItem::MacroInvocation(span, _) => span,
            UnexpandedItem::TypeMacro(span, _) => span,
            UnexpandedItem::AttributeMacro(span, _) => span,
            UnexpandedItem::DeriveMacro(span, _) => span,
            UnexpandedItem::UnexpandedModule { span, .. } => span,
            UnexpandedItem::MacroUse(span, _) => span,
        }
    }
}

/// A cursor examining an unexpanded module.
pub struct UnexpandedCursor<'a> {
    pub module: &'a mut UnexpandedModule,
    idx: usize,
}
impl<'a> UnexpandedCursor<'a> {
    /// Crate a cursor into a module.
    pub fn new(module: &'a mut UnexpandedModule) -> UnexpandedCursor<'a> {
        let idx = module.items.len();
        UnexpandedCursor { module, idx }
    }
    /// Insert something into the module.
    pub fn insert(&mut self, item: UnexpandedItem) {
        self.module.items.insert(self.idx, item);
        self.idx += 1;
    }
    /// Reset to the front of the target module.
    pub fn reset(&mut self) {
        self.idx = 0;
    }
    /// Pop the item at the cursor position.
    pub fn pop(&mut self) -> Option<UnexpandedItem> {
        if self.module.items.len() <= self.idx {
            None
        } else {
            Some(self.module.items.remove(self.idx))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::macros::lower_macro_rules;
    use quote::{quote, ToTokens};

    #[test]
    fn full_macro() {
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
        test_ctx!(ctx);

        let rules: syn::ItemMacro = syn::parse_quote! { macro_rules! test_macro {
            () => (hooray);
        }};
        let rules = lower_macro_rules(&ctx, &rules).unwrap();

        let input = quote!();

        let output = apply_once(&rules, input).unwrap();

        assert_eq!(output.to_string(), quote!(hooray).to_string());
    }
}
