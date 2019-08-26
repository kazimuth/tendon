//! Walk through a module, feeding data to syn and then a `resolver::Db`.
//! Expands macros as it goes.

use super::namespace::Namespace;
use super::{ModuleImports, ResolveError};
use crate::{resolver::Db, Map};
use cargo_metadata::{CargoOpt, Node, Package, PackageId};
use parking_lot::RwLock;
use quote::ToTokens;
use std::fs::File;
use std::io::Read;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use syn;
use tracing::{error, info, info_span, trace};
use transgress_api::attributes::{Metadata, Span};
use transgress_api::idents::Ident;
use transgress_api::items::{DeclarativeMacroItem, MacroItem, ModuleItem, SymbolItem, TypeItem};
use transgress_api::paths::{AbsoluteCrate, AbsolutePath, Path};
use transgress_api::tokens::Tokens;

pub fn walk_crate(
    crate_: AbsoluteCrate,
    root: &FsPath,
    external_macros: &Namespace<MacroItem>,
) -> Result<Db, ResolveError> {
    unimplemented!()
}

/// Parse a module into a database.
pub fn walk_mod(
    crate_root: &FsPath,
    mod_: &AbsolutePath,
    attrs: &[syn::Attribute],
    items: &[syn::Item],
    extern_macros: &Namespace<MacroItem>,
    extern_crates: &Map<Ident, AbsoluteCrate>,
    local_macros: Map<Ident, DeclarativeMacroItem>,
) -> Result<(Db, Map<Ident, DeclarativeMacroItem>), ResolveError> {
    trace!("lowering {:?}", mod_);
    let mut result = Db::new();
    //let mut macro_results = vec![];
    let mut scope = ModuleImports::new();

    // First pass: locate imports
    //
    for item in items {
        match item {
            syn::Item::Use(use_) => {}
            _ => (),
        }
    }

    // Second pass: locate macro and module definitions; walk `macro_use` modules as they are discovered

    // Invoke parallelizable modules in parallel

    // Third pass: lower all items

    // Fourth pass: merge child Dbs

    for item in items {
        match item {
            syn::Item::Mod(module_) => skip("mod", mod_.clone().join(&module_.ident)),
            syn::Item::ExternCrate(crate_) => skip("crate", mod_.clone().join(&crate_.ident)),
            syn::Item::Static(static_) => skip("static", mod_.clone().join(&static_.ident)),
            syn::Item::Const(const_) => skip("const", mod_.clone().join(&const_.ident)),
            syn::Item::Fn(fn_) => skip("fn", mod_.clone().join(&fn_.sig.ident)),
            syn::Item::Type(type_) => skip("type", mod_.clone().join(&type_.ident)),
            syn::Item::Struct(struct_) => skip("struct", mod_.clone().join(&struct_.ident)),
            syn::Item::Enum(enum_) => skip("enum", mod_.clone().join(&enum_.ident)),
            syn::Item::Union(union_) => skip("union", mod_.clone().join(&union_.ident)),
            syn::Item::Trait(trait_) => skip("trait", mod_.clone().join(&trait_.ident)),
            syn::Item::TraitAlias(alias_) => skip("alias", mod_.clone().join(&alias_.ident)),
            syn::Item::Impl(impl_) => {
                info!("impl: {}", impl_.into_token_stream());
            }
            syn::Item::Macro(macro_rules_) => {
                if let Some(ident) = &macro_rules_.ident {
                    skip("macro_rules", mod_.clone().join(ident))
                }
            }
            syn::Item::Macro2(macro2_) => skip("macro2_", mod_.clone().join(&macro2_.ident)),
            syn::Item::ForeignMod(_foreign_mod_) => skip("foreign_mod", mod_.clone()),
            syn::Item::Verbatim(_verbatim_) => skip("verbatim", mod_.clone()),
            _ => skip("something else", mod_.clone()),
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
    unimplemented!();
    //Ok(result)
}

pub fn parse_file(file: &FsPath) -> Result<syn::File, ResolveError> {
    trace!("parsing `{}`", file.display());

    let mut file = File::open(file)?;
    let mut source = String::new();
    file.read_to_string(&mut source)?;

    Ok(syn::parse_file(&source)?)
}

pub fn skip(kind: &str, path: AbsolutePath) {
    info!("skipping {} {:?}", kind, &path);
}
