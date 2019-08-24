/*
//! Namespaces.
use transgress_api::paths::{AbsolutePath, AbsoluteCrate, Path};
use transgress_api::idents::Ident;
use transgress_api::tokens::Tokens;
use transgress_api::items::{TypeItem, SymbolItem, MacroItem, ModuleItem};
use transgress_api::attributes::{Span, Metadata};
use crate::{Map};
use cargo_metadata::{CargoOpt, Node, Package, PackageId};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use syn;
use quote::ToTokens;
use tracing::{info, info_span, error};
use parking_lot::RwLock;

pub mod resolvable;
pub mod namespace;

use namespace::Namespace;

/// A database of all known paths and their contents.
pub struct Db {
    pub types: Namespace<TypeItem>,
    pub symbols: Namespace<SymbolItem>,
    pub macros: Namespace<MacroItem>,
    pub modules: Namespace<ModuleItem>,
    pub scopes: Namespace<Scope>,
}

// A scope.
// Each scope currently corresponds to a module; that might change if we end up having to handle
// impl's in function scopes.
pub struct Scope {
    /// This module's glob imports.
    /// `use x::y::z::*` is stored as `x::y::z` pre-resolution,
    /// and as an AbsolutePath post-resolution.
    /// Includes the prelude, if any.
    /// These aren't guaranteed to be resolved! We resolve as we go :)
    pub glob_imports: Vec<Path>,

    /// This module's non-glob imports.
    /// Maps the imported-as ident to a path,
    /// i.e. `use x::Y;` is stored as `Y => x::Y`,
    /// `use x::z as w` is stored as `w => x::z`
    pub imports: Map<Ident, Path>,
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

/// Parse a module into a database.
pub fn parse_mod(mod_: &AbsolutePath, root: PathBuf) -> Result<Db, ResolveError> {
    info!("parsing {:?} (`{}`)", mod_, root.display());

    let mut file = File::open(root)?;
    let mut source = String::new();
    file.read_to_string(&mut source)?;

    let source = syn::parse_file(&source)?;

    let mut result = Db::new();

    for item in &source.items {
        match item {
            syn::Item::Mod(module_) => skip("mod", mod_.join(&module_.ident)),
            syn::Item::Use(use_) => {
                info!("use {}", use_.tree.clone().into_token_stream());
            }
            syn::Item::ExternCrate(crate_) => skip("crate", mod_.join(&crate_.ident)),
            syn::Item::Static(static_) => skip("static", mod_.join(&static_.ident)),
            syn::Item::Const(const_) => skip("const", mod_.join(&const_.ident)),
            syn::Item::Fn(fn_) => skip("fn", mod_.join(&fn_.ident)),
            syn::Item::Type(type_) => skip("type", mod_.join(&type_.ident)),
            syn::Item::Existential(existential_) => {
                skip("existential", mod_.join(&existential_.ident))
            }
            syn::Item::Struct(struct_) => skip("struct", mod_.join(&struct_.ident)),
            syn::Item::Enum(enum_) => skip("enum", mod_.join(&enum_.ident)),
            syn::Item::Union(union_) => skip("union", mod_.join(&union_.ident)),
            syn::Item::Trait(trait_) => skip("trait", mod_.join(&trait_.ident)),
            syn::Item::TraitAlias(alias_) => skip("alias", mod_.join(&alias_.ident)),
            syn::Item::Impl(impl_) => {
                info!("impl: {}", impl_.into_token_stream());
            }
            syn::Item::Macro(macro_rules_) => {
                if let Some(ident) = &macro_rules_.ident {
                    skip("macro_rules", mod_.join(ident))
                }
            }
            syn::Item::Macro2(macro2_) => skip("macro2_", mod_.join(&macro2_.ident)),
            syn::Item::ForeignMod(_foreign_mod_) => skip("foreign_mod", mod_.clone()),
            syn::Item::Verbatim(_verbatim_) => skip("verbatim", mod_.clone()),
        }
    }

    // scope to target crate?

    // https://doc.rust-lang.org/stable/reference/items/modules.html
    // mod q; -> q/mod.rs; q.rs
    //
    // #[path="z.rs"] mod q -> z.rs
    // #[path="bees"] mod wasps { mod queen; } -> bees/queen.rs, bees/queen/mod.rs

    // cfg-attrs

    // prelude

    // check edition
    Ok(result)
}

pub fn skip(kind: &str, path: AbsolutePath) {
    info!("skipping {} {:?}", kind, &path);
}
*/
