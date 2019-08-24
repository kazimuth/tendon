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

pub mod ast;
pub mod consume;
pub mod transcribe;

quick_error! {
    #[derive(Debug)]
    pub enum ExpandError {

    }
}
