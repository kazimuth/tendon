//! Lowering operations, from syn's syntax tree to transgress-APIs items (and their components).
//! Implemented parser-combinator style, with code split out to helpers.
//! Note: syn's datastructure's aren't thread-safe, so we can never include them in the output data.
//! Style note: always prefix syn types with "syn" in this crate.

// TODO trait lowering: https://rust-lang.github.io/rustc-guide/traits/index.html
//  rules reference: https://rust-lang.github.io/rustc-guide/traits/lowering-rules.html

use std::path::PathBuf;
use transgress_api::{
    attributes::{Span, Visibility},
    tokens::Tokens,
};

#[cfg(test)]
/// Helper macro to make working with match trees easier in tests.
macro_rules! assert_match {
    ($arg:expr, $binding:pat $(=> $rest:expr)?) => {
        let ref arg = $arg;
        match arg {
            $binding => {
                $($rest)?
            },
            _ => panic!("failed to match {:?} to {}", arg, stringify!($binding))
        }
    }
}

pub mod attributes;
pub mod struct_;
pub mod types;

/// Context for lowering items in an individual module.
pub struct ModuleCtx {
    /// The location of this module's containing file in the filesystem.
    source_file: PathBuf,
    /// The visibility of this module.
    visibility: Visibility,
}

quick_error! {
    #[derive(Debug)]
    pub enum LowerError {
        /// lol no generics
        NoGenericsYet(span: Span) {
            display("generics unimplemented, can't lower (at {:?})", span)
        }
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
    }
}
