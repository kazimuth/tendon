//! Namespaces.
use crate::{lower::LowerError, Map};
use std::path::Path as FsPath;
use syn;
use transgress_api::idents::Ident;
use transgress_api::items::{MacroItem, ModuleItem, SymbolItem, TypeItem};
use transgress_api::paths::{AbsoluteCrate, AbsolutePath, Path};

#[cfg(test)]
macro_rules! test_ctx {
    ($ctx:ident) => {
        let source_file = std::path::PathBuf::from("fake_file.rs");
        let module = transgress_api::paths::AbsolutePath {
            crate_: transgress_api::paths::AbsoluteCrate {
                name: "fake_crate".into(),
                version: "0.0.1".into(),
            },
            path: vec![],
        };
        let root_db = crate::resolver::Db::new();
        let crate_map = crate::Map::default();

        let $ctx = ModuleCtx {
            source_file: &source_file,
            module: &module,
            root_db: &root_db,
            crate_map: &crate_map,
        };
    };
}

pub mod namespace;
pub mod resolvable;
pub mod walker;

use namespace::Namespace;

/// A database of all known paths and their contents.
pub struct Db {
    pub types: Namespace<TypeItem>,
    pub symbols: Namespace<SymbolItem>,
    pub macros: Namespace<MacroItem>,
    /// `mod` items, mostly just store metadata.
    pub modules: Namespace<ModuleItem>,
    /// Scopes; used in name resolution, then discarded.
    pub scopes: Namespace<ModuleImports>,
}

impl Db {
    /// Create a new database.
    pub fn new() -> Db {
        Db {
            types: Namespace::new(),
            symbols: Namespace::new(),
            macros: Namespace::new(),
            modules: Namespace::new(),
            scopes: Namespace::new(),
        }
    }

    /// Add all entries from another database.
    /// Collisions will be ignored with a warning.
    pub fn merge_from(&mut self, other: Db) {
        let Db {
            types,
            symbols,
            macros,
            modules,
            scopes,
        } = other;

        self.types.merge_from(types);
        self.symbols.merge_from(symbols);
        self.macros.merge_from(macros);
        self.modules.merge_from(modules);
        self.scopes.merge_from(scopes);
    }
}

/// Context for lowering items in an individual module.
pub struct ModuleCtx<'a> {
    /// The location of this module's containing file in the filesystem.
    pub source_file: &'a FsPath,
    /// The module path.
    pub module: &'a AbsolutePath,
    /// A Db containing resolved definitions for all dependencies.
    pub root_db: &'a Db,
    /// Names for external crates in this module.
    pub crate_map: &'a Map<Ident, AbsoluteCrate>,
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

    /// This module's `pub` glob imports.
    /// `use x::y::z::*` is stored as `x::y::z` pre-resolution,
    /// and as an AbsolutePath post-resolution.
    /// Includes the prelude, if any.
    pub pub_glob_imports: Vec<Path>,

    /// This module's non-glob `pub` imports.
    /// Maps the imported-as ident to a path,
    /// i.e. `use x::Y;` is stored as `Y => x::Y`,
    /// `use x::z as w` is stored as `w => x::z`
    pub pub_imports: Map<Ident, Path>,
}

impl ModuleImports {
    /// Create a new set of imports
    fn new() -> ModuleImports {
        ModuleImports {
            glob_imports: Vec::new(),
            imports: Map::default(),
            pub_glob_imports: Vec::new(),
            pub_imports: Map::default(),
        }
    }

    /*
    /// Resolve paths used in imports
    fn resolve(&mut self) {
        // TODO: how to handle root scope?

    }
    */
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
