//! This crate implements API resolution for Rust:
//! that is, enough of Rust's parsing, macro expansion, and name resolution to generate a full
//! description of a crate's API. This description (in the crate `tendon_api`) is then used
//! to generate bindings down the line.
//!
//! ## Algorithm
//! The algorithm used is fairly simple in concept, if a bit hairy in execution. It performs lazy,
//! fault-tolerant parsing and name resolution on a dependency graph of rust crates. Lazy to avoid
//! doing work we don't need to do -- there's no point in resolving anything that isn't used by
//! a crate's public interface. And fault-tolerant because rust is a big language, and we don't
//! want to block codegen on features we haven't implemented yet. The `cargo_metadata` crate is used
//! to find source code in the filesystem, and the `syn` crate is used for parsing.
//!
//! The core of the algorithm is the `Db`: a map from `tendon_api::AbsolutePath`s to partially-
//! resolved items -- structs, functions, macros, etc., containing resolved and unresolved paths.
//!
//! During each step of the algorithm, a thread walks over all unresolved paths in every item,
//! attempting to resolve them in-place as it goes. if the path referenced by the item exists in the
//! `Db`, the path is converted from an `UnresolvedPath` to an `AbsolutePath`. If the path can't be found,
//! the walking thread queues up a request to parse the file it could be found in. Once all items have been walked,
//! a thread pool parses all the requested files, adding their contents to the `Db`. Then the
//! algorithm repeats, until all paths have either been resolved or marked unresolvable.

// TODO remove
#![allow(dead_code)]

#[macro_use]
extern crate quick_error;

/*
mod syn {
    pub struct Visibility;
    pub struct Bananas;
}

macro_rules! z {
() => (
mod q {
    pub fn t(x: &crate::walker::ModuleScope) -> syn::Bananas {
        if let syn::Visibility::Public(_) = x {
            panic!()
        }
        panic!()
    }
}
);
}
z!();
use q::t;
*/

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

#[macro_use]
pub mod walker;
pub mod lower;
pub mod macro_interp;
pub mod namespace;
pub mod resolver;
pub mod tools;
