//! Namespaces.
use crate::{lower::LowerError, Map};
use syn;
use transgress_api::idents::Ident;
use transgress_api::items::{MacroItem, ModuleItem, SymbolItem, TypeItem};
use transgress_api::paths::{AbsoluteCrate, AbsolutePath, Path, UnresolvedPath};

pub mod namespace;
pub mod resolvable;
pub mod walker;

use namespace::Namespace;

/// A database of all known paths and their contents.
pub struct Db {
    pub types: Namespace<TypeItem>,
    pub symbols: Namespace<SymbolItem>,
    pub macros: Namespace<MacroItem>,
    pub modules: Namespace<ModuleItem>,
    pub scopes: Namespace<ModuleImports>,
}

// macro name resolution is affected by order, right?
//
// see: https://danielkeep.github.io/tlborm/book/mbe-min-scoping.html
//
// other stuff:
// see: https://rust-lang.github.io/rustc-guide/name-resolution.html
// see: https://github.com/rust-lang/rust/blob/master/src/librustc_resolve/lib.rs
// see: https://github.com/rust-lang/rfcs/blob/master/text/1560-name-resolution.md (not yet implemented)
// see: https://doc.rust-lang.org/edition-guide/rust-2018/macros/macro-changes.html
//
// note: don't type uses, allow passthrough (actually the better choice anyway)
//
// TODO #[macro_use] imports
// TODO prelude
// TODO: is_safe_for_auto_derive -- trait has no type members
// TODO: handle rust edition

// https://github.com/rust-lang/rustc-guide/blob/master/src/name-resolution.md
// https://doc.rust-lang.org/reference/items/extern-crates.html
// > When naming Rust crates, hyphens are disallowed. However, Cargo packages may make use of them.
// > In such case, when Cargo.toml doesn't specify a crate name, Cargo will transparently replace -
// > with _ (Refer to RFC 940 for more details).

// alg:
//     walk paths
//     parse
//     resolve macros (via use thing::macro, macro_use)
//     expand macros
//     parse new data, walk, expand macros until done
//     resolve everything else

quick_error! {
    #[derive(Debug)]
    pub enum ResolveError {
        Io(err: std::io::Error) {
            from()
            cause(err)
            description(err.description())
            display("io error during resolution: {}", err)
        }
        Parse(err: syn::Error) {
            from()
            cause(err)
            description(err.description())
            display("parse error during resolution: {}", err)
        }
        PathNotFound(namespace: &'static str, path: AbsolutePath) {
            display("path {:?} not found in {} namespace", path, namespace)
        }
        AlreadyDefined(namespace: &'static str, path: AbsolutePath) {
            display("path {:?} already defined in {} namespace", path, namespace)
        }
        Lower(err: LowerError) {
            from()
            cause(err)
            description(err.description())
            display("{}", err)
        }
        CachedError(path: AbsolutePath) {
            display("path {:?} is invalid due to some previous error", path)
        }
    }
}

impl Db {
    pub fn new() -> Db {
        Db {
            types: Namespace::new(),
            symbols: Namespace::new(),
            macros: Namespace::new(),
            modules: Namespace::new(),
            scopes: Namespace::new(),
        }
    }
}

// A scope.
// Each scope currently corresponds to a module; that might change if we end up having to handle
// impl's in function scopes.
pub struct ModuleImports {
    /// This module's glob imports.
    /// `use x::y::z::*` is stored as `x::y::z` pre-resolution,
    /// and as an AbsolutePath post-resolution.
    /// Includes the prelude, if any.
    pub glob_imports: Vec<Path>,

    /// This module's non-glob imports.
    /// Maps the imported-as ident to a path,
    /// i.e. `use x::Y;` is stored as `Y => x::Y`,
    /// `use x::z as w` is stored as `w => x::z`
    pub imports: Map<Ident, Path>,
}

impl ModuleImports {
    fn new() -> ModuleImports {
        ModuleImports {
            glob_imports: Vec::new(),
            imports: Map::default(),
        }
    }
}

/*
pub struct CrateAdaptor<'a> {
    crate_: AbsoluteCrate,
    scopes: &'a Db
}

impl CrateAdaptor<'_> {
    fn lookup(&self, module: path: UnresolvedPath)
}
*/
