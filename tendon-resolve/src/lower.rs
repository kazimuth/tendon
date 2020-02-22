//! Lowering operations, from syn's syntax tree to tendon-APIs items (and their components).
//! Implemented parser-combinator style, with code split out to helpers.
//! Note: syn's datastructure's aren't thread-safe, so we can never include them in the output data.
//! Style note: always prefix syn types with "syn" in this crate.

use std::fmt;
use tendon_api::tokens::Tokens;

pub mod attributes;
pub mod generics;
pub mod items;
pub mod macros;
pub mod modules;
pub mod types;

//pub mod imports;

quick_error! {
    pub enum LowerError {
        NoHRTBsYet(hrtb_: Tokens) {
            display("HRTBs unimplemented, can't lower {:?}", hrtb_)
        }
        UnhandledType(type_: Tokens) {
            display("i don't know how to lower the type {:?}", type_)
        }
        UnexpectedGenericInPath(path: Tokens) {
            display("path {:?} contains unexpected generic", path)
        }
        MalformedType(type_: Tokens, meta: &'static str) {
            display("malformed type {:?}: {}", type_, meta)
        }
        MalformedPredicate(predicate: Tokens) {
            display("malformed `where` predicate: {:?}", predicate)
        }
        MalformedFunctionArg(arg: Tokens) {
            display("malformed function argument: {:?}", arg)
        }
        NotAMacroDeclaration {
            display("not a macro declaration?")
        }
        TypePositionMacro {
            display("type-position macro")
        }
        CfgdOut {
            display("item is #[cfg]'d out")
        }
    }
}

impl fmt::Debug for LowerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
