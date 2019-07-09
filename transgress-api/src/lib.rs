//! Simple datastructures describing a rust program's interface: types, function signatures, consts, etc.
//! Produced and consumed by other `transgress` crates.
//!
//! Some inspiration taken from https://github.com/rust-lang/rls/tree/master/rls-data, although we represent
//! a significantly smaller subset of rust program metadata.
//!
//! ### Why not just use syn?
//! Syn is a syntax tree which includes all the information needed to reconstruct the textual program input;
//! we don't need that. We include a streamlined set of data designed to be used by binding generators.
//! In addition, syn's types aren't Send or Serialize which is a pain.
//!
//! N.B.: There are a couple places where we just include strings here designed to be parsed by syn.
//!
//! ### References
//! - [Rust attributes](https://doc.rust-lang.org/reference/attributes.html)
//! - [Name resolution](https://rust-lang.github.io/rustc-guide/name-resolution.html)
//! - [Paths](https://doc.rust-lang.org/stable/reference/paths.html)

// TODO: impls on non-local types??
//       can do custom handling for e.g. IntoIterator for now
// TODO: constexprs grumble grumble

//
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub mod attributes;
pub mod expr;
pub mod ident;
pub mod items;
pub mod paths;
pub mod traits;
pub mod types;

pub use expr::{ConstExpr, Expr};
pub use ident::Ident;
pub use paths::Path;
pub use traits::Trait;
pub use types::Type;
