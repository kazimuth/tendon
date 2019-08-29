//! Walk through a module, feeding data to syn and then a `resolver::Db`.
//! Expands macros as it goes.

use super::namespace::Namespace;
use super::{ModuleCtx, ModuleImports, ResolveError};
use crate::{
    expand::apply_once,
    lower::{
        attributes::lower_visibility,
        imports::lower_use,
        macros::{lower_macro_rules, MACRO_RULES},
        modules::lower_module,
    },
    resolver::Db,
    Map,
};
use quote::ToTokens;
use std::fmt::Display;
use std::fs::File;
use std::io::Read;
use std::path::{Path as FsPath, PathBuf};
use syn::spanned::Spanned;
use tracing::{info, trace, warn};
use transgress_api::attributes::{Span, Visibility};
use transgress_api::idents::Ident;
use transgress_api::items::{DeclarativeMacroItem, MacroItem, ModuleItem};
use transgress_api::paths::{AbsoluteCrate, AbsolutePath, Path};

/*
pub fn walk_crate(
    crate_: AbsoluteCrate,
    root: &FsPath,
    external_macros: &Namespace<MacroItem>,
) -> Result<Db, ResolveError> {
    unimplemented!()
}

macro_rules! unwrap_or_warn {
    ($result:expr, $span:expr) => (
        match $result {
            Ok(result) => result,
            Err(err) => {
                warn($span, &err);
                continue
            }
        }
    );
}

// ** TODO **: refactor `ModuleCtx` to include:
// - extern macros and crates (reference)
// - reference to local_macros map
// - mut reference to DB
// - source_file
// - pending modules?
//
// have walk_items take a module ctx and return a local_macros map
//
// split out phases, and keep lists of TODOs w/ separate phases?
//      to increase latent parallelism e.g. by parsing macros from modules immediately?
//              nah, unlikely case
//      run macro lowering immediately w/ current DB
//

/// Parse a set of items into a database.
pub fn walk_items(
    source_file: &FsPath,
    mod_: &AbsolutePath,
    items: &[syn::Item],
    extern_macros: &Namespace<MacroItem>,
    extern_crates: &Map<Ident, AbsoluteCrate>,
    parent_macros: &Map<Ident, DeclarativeMacroItem>,
) -> Result<(Db, Map<Ident, DeclarativeMacroItem>), ResolveError> {

    // TODO: this could probably be refactored to be moderately more sane, but eh

    trace!("lowering {:?}", mod_);
    let mut db = Db::new();
    let mut scope = ModuleImports::new();
    let mut local_macros = Map::new();

    let ctx = ModuleCtx {
        source_file: source_file.to_owned()
    };

    let make_span = |thing: &dyn Spanned| {
        Span::from_syn(source_file.to_owned(), thing.span())
    };

    // First pass: locate imports
    for item in items {
        match item {
            syn::Item::Use(use_) => {
                if lower_visibility(&use_.vis) == Visibility::Pub {
                    lower_use(&use_, &mut scope.pub_glob_imports, &mut scope.pub_imports);
                } else {
                    lower_use(&use_, &mut scope.glob_imports, &mut scope.imports);
                }
            }
            _ => (),
        }
    }

    let mut parallel_mods: Vec<(ModuleItem, AbsolutePath, Option<PathBuf>)> = vec![];

    // Second pass: handle macros and module definitions;
    // walk `macro_use` modules as they are discovered, store other modules to walk in parallel later

    // TODO: break out?
    for item in items {
        if let syn::Item::Macro(macro_) = item {
            let span = &make_span(macro_);
            let path = Path::from(&macro_.mac.path);
            if &path == &*MACRO_RULES {
                let macro_lowered = lower_macro_rules(&ctx, macro_);
                let macro_lowered = unwrap_or_warn!(macro_lowered, span);
                if macro_lowered.macro_export {
                    // export the macro outside this crate, at $crate::macro_name.
                    // note that this exported macro isn't used while parsing the current
                    // crate.
                    let path = AbsolutePath {
                        crate_: mod_.crate_.clone(),
                        path: vec![macro_lowered.name.clone()]
                    };
                    // TODO: handle merging macros
                    db.macros.insert(path, MacroItem::Declarative(macro_lowered.clone()));
                }
                // TODO: handle merging macros
                local_macros.insert(macro_lowered.name.clone(), macro_lowered);
            } else {
                let decl = if extern_
                    if let Some(ident) = path.get_ident() {
                    if let Some(decl) = local_macros.get(&ident[..]).or_else(|| parent_macros.get(&ident[..])) {
                        decl
                    }

                    warn(
                        &Span::from_syn(source_file.to_owned(), macro_.span()),
                        format!("macro not found: {}!", ident)
                    );
                    unimplemented!()
                };
                let invoked = unwrap_or_warn!(apply_once(decl, macro_.mac.tokens.clone()), span);
                let file = unwrap_or_warn!(syn::parse2::<syn::File>(invoked), span);

                // walk generated items recursively
                // TODO recursion depth?
                let result = walk_items(
                    // TODO this will give wrong line#s in generated macro
                    &source_file,
                                mod_,
                                &file.items,
                                extern_macros,
                                extern_crates,
                           local_macros.clone()
                );
                let (macro_db, macro_macros) = unwrap_or_warn!(result);
                // TODO merge

            }
        } else if let syn::Item::Mod(module) = item {
            let lowered = lower_module(&ctx, module);
            if let Some((brace, items)) = module.content {
                // TODO merge
                let result = walk_items(source_file, &mod_.join(&lowered.ident), &items, extern_macros, extern_crates, local_macros.clone())?;
            }
        }
    }

    // Invoke parallelizable modules in parallel

    // Third pass: lower all items

    for item in items {
        match item {
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
            syn::Item::ForeignMod(_foreign_mod_) => skip("foreign_mod", mod_.clone()),
            syn::Item::Verbatim(_verbatim_) => skip("verbatim", mod_.clone()),
            _ => skip("something else", mod_.clone()),
        }
    }

    // Fourth pass: merge child Dbs


    // scope to target crate?

    // https://doc.rust-lang.org/stable/reference/items/modules.html
    // mod q; -> q/mod.rs; q.rs
    //
    // #[path="z.rs"] mod q -> z.rs
    // #[path="bees"] mod wasps { mod queen; } -> bees/queen.rs, bees/queen/mod.rs

    // cfg-attrs

    // prelude

    // check edition
    Ok((db, local_macros))
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

pub fn warn(span: &Span, cause: &dyn Display) {
    warn!("ignoring error [{:?}]: {}", span, cause);
}
*/
