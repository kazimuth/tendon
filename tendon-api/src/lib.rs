//! Simple data structures describing a rust program's interface: types, function signatures, consts, etc.
//! Produced and consumed by other `tendon` crates.
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
//! - [Name resolution impl](https://github.com/rust-lang/rust/blob/master/src/librustc_resolve/lib.rs)
//! - [Paths](https://doc.rust-lang.org/stable/reference/paths.html)

#[allow(unused)]
#[macro_use]
extern crate quick_error;

#[macro_use]
extern crate lazy_static;

#[allow(unused)]
macro_rules! debug {
    ($name:ty, $fmt:literal $(, $arg:ident)*) => (
        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, $fmt $(, self.$arg)*)
            }
        }
    )
}

/// Fast single-thread-writeable maps.
pub type Map<K, V> = hashbrown::HashMap<K, V, fxhash::FxBuildHasher>;
/// Fast single-thread-writeable sets.
pub type Set<K> = hashbrown::HashSet<K, fxhash::FxBuildHasher>;

#[macro_use]
pub mod attributes;
pub mod crates;
pub mod database;
pub mod expressions;
pub mod generics;
pub mod identities;
pub mod idents;
pub mod items;
pub mod paths;
pub mod scopes;
pub mod tokens;
pub mod types;
