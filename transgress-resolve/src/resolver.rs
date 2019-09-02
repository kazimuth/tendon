use crate::{lower::LowerError, Map};
use std::path::PathBuf;
use syn;
use transgress_api::idents::Ident;
use transgress_api::items::{MacroItem, ModuleItem, SymbolItem, TypeItem};
use transgress_api::paths::{AbsoluteCrate, AbsolutePath, Path};
use transgress_api::tokens::Tokens;
use dashmap::DashMap;
use namespace::Namespace;
use transgress_api::attributes::Span;


// https://github.com/rust-lang/rust/tree/master/src/librustc_resolve

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
        let db = crate::resolver::Db::new();
        let mut scope = crate::resolver::ModuleImports::new();
        let mut unexpanded = crate::resolver::UnexpandedModule::new();
        let crate_unexpanded_modules = dashmap::DashMap::default();
        let source_root = std::path::PathBuf::from("fake_src/");

        let $ctx = ModuleCtx {
            source_file,
            module,
            db: &db,
            scope: &mut scope,
            unexpanded: &mut unexpanded,
            crate_unexpanded_modules: &crate_unexpanded_modules,
            source_root: &source_root
        };
    };
}

pub mod namespace;
pub mod resolvable;
pub mod walker;

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
}

/// Context for lowering items in an individual module.
pub struct ModuleCtx<'a> {
    /// The location of this module's containing file in the filesystem.
    pub source_file: PathBuf,
    /// The module path.
    pub module: AbsolutePath,

    /// A Db containing resolved definitions for all dependencies.
    pub db: &'a Db,

    /// The scope for this module.
    pub scope: &'a mut ModuleImports,

    /// All items in this module that need to be macro-expanded.
    pub unexpanded: &'a mut UnexpandedModule,

    /// Unexpanded modules in this crate.
    pub crate_unexpanded_modules: &'a DashMap<AbsolutePath, UnexpandedModule>,

    /// The source root (i.e. directory containing root lib.rs file) of this crate
    pub source_root: &'a PathBuf
}

/// Metadata for a crate instantiation. There's one of these for every separate semver version for
/// every crate in the dependency tree.
#[derive(Debug)]
pub struct CrateData {
    /// The dependencies of this crate (note: renamed according to Cargo.toml, but NOT according to
    /// `extern crate ... as ...;` statements
    pub deps: Map<Ident, AbsoluteCrate>,
    /// The *activated* features of this crate.
    pub features: Vec<String>,
    /// The path to the crate's `Cargo.toml`.
    pub manifest_path: PathBuf,
    /// The entry file into the crate.
    /// Note that this isn't always `crate_root/src/lib.rs`, some crates do other wacky stuff.
    pub entry: PathBuf,
    /// The source this crate was downloaded from.
    /// If not present, the crate is a local dependency and must be referred to by relative path.
    pub cargo_source: Option<cargo_metadata::Source>,
    /// If this crate is a proc-macro crate.
    pub is_proc_macro: bool,
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
        MalformedPathAttribute(tokens: String) {
            display("malformed `#[path]` attribute: {}", tokens)
        }
        Root {
            display("files at fs root??")
        }
        ModuleNotFound {
            display("couldn't find source file")
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
    pub fn new() -> ModuleImports {
        ModuleImports {
            glob_imports: Vec::new(),
            imports: Map::default(),
            pub_glob_imports: Vec::new(),
            pub_imports: Map::default(),
        }
    }
}

/// A module with macros unexpanded.
/// We throw all macro-related stuff here when we're walking freshly-parsed modules.
/// It's not possible to eagerly expand macros because they rely on name resolution to work, and we
/// can't do name resolution (afaict) until after we've lowered most modules already.
/// This is ordered because order affects macro name resolution.
pub struct UnexpandedModule(Vec<UnexpandedItem>);
impl UnexpandedModule {
    /// Create an empty unexpanded module.
    pub fn new() -> Self {
        UnexpandedModule(vec![])
    }
}

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
    UnexpandedModule { name: Ident, macro_use: bool }
}

