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
use std::fs::{File};
use std::io::Read;
use std::path::{Path as FsPath, PathBuf};
use syn::spanned::Spanned;
use tracing::{info, trace, warn};
use transgress_api::attributes::{Span, Visibility};
use transgress_api::idents::Ident;
use transgress_api::items::{DeclarativeMacroItem, MacroItem, ModuleItem};
use transgress_api::paths::{AbsoluteCrate, AbsolutePath, Path};
use lazy_static::lazy_static;
use std::mem;

lazy_static! {
    static ref MACRO_USE: Path = Path::fake("macro_use");
    static ref PATH: Path = Path::fake("path");
}

/*
pub fn walk_crate(
    crate_: AbsoluteCrate,
    root: &FsPath,
    external_macros: &Namespace<MacroItem>,
) -> Result<Db, ResolveError> {
    unimplemented!()
}
*/

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
    ctx: &mut ModuleCtx,
    items: &[syn::Item],
) -> Result<Db, ResolveError> {

    // TODO: this could probably be refactored to be moderately more sane, but eh

    trace!("lowering {:?}", ctx.module);
    let mut db = Db::new();

    // First pass: locate imports
    for item in items {
        match item {
            syn::Item::Use(use_) => {
                if lower_visibility(&use_.vis) == Visibility::Pub {
                    lower_use(&use_, &mut ctx.scope.pub_glob_imports, &mut ctx.scope.pub_imports);
                } else {
                    lower_use(&use_, &mut ctx.scope.glob_imports, &mut ctx.scope.imports);
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
            let span = &Span::from_syn(ctx.source_file.to_owned(), macro_.span());
            let path = Path::from(&macro_.mac.path);
            if &path == &*MACRO_RULES {
                let macro_lowered = lower_macro_rules(&ctx, macro_);
                let macro_lowered = unwrap_or_warn!(macro_lowered, span);
                if macro_lowered.macro_export {
                    // Export the macro outside this crate, at $crate::macro_name.
                    // Note: this exported macro *isn't used* while parsing the current
                    // crate.
                    let path = AbsolutePath {
                        crate_: ctx.module.crate_.clone(),
                        path: vec![macro_lowered.name.clone()]
                    };
                    // TODO: handle merging macros
                    db.macros.insert(path, MacroItem::Declarative(macro_lowered.clone()));
                }
                // TODO: handle merging macros
                ctx.local_macros.insert(macro_lowered.name.clone(), macro_lowered);
            } else {
                /*
                let decl = if extern_
                    if let Some(ident) = path.get_ident() {
                    if let Some(decl) = local_macros.get(&ident[..]).or_else(|| local_macros.get(&ident[..])) {
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
                                ctx.module,
                                &file.items,
                                extern_macros,
                                extern_crates,
                           local_macros.clone()
                );
                let (macro_db, macro_macros) = unwrap_or_warn!(result);
                */
                panic!()
                // TODO merge

            }
        } else if let syn::Item::Mod(module) = item {
            let mut lowered = lower_module(&ctx, module);
            let items = if let Some((_, items)) = &module.content {
                items
            } else if lowered.metadata.extract_attribute(&MACRO_USE).is_some() {

            } else { continue };

            let mut new_module = ctx.module.clone().join(lowered.name.clone());
            let span = &Span::from_syn(ctx.source_file.to_owned(), module.span());

            // patch current context to walk child mod
            mem::swap(&mut ctx.module, &mut new_module);
            let result = walk_items(ctx, &items);
            mem::swap(&mut ctx.module, &mut new_module);

            let child_db = unwrap_or_warn!(result, &span);
            db.merge_from(child_db);
        }
    }

    // Invoke parallelizable modules in parallel

    // Third pass: lower all items

    for item in items {
        match item {
            syn::Item::Static(static_) => skip("static", ctx.module.clone().join(&static_.ident)),
            syn::Item::Const(const_) => skip("const", ctx.module.clone().join(&const_.ident)),
            syn::Item::Fn(fn_) => skip("fn", ctx.module.clone().join(&fn_.sig.ident)),
            syn::Item::Type(type_) => skip("type", ctx.module.clone().join(&type_.ident)),
            syn::Item::Struct(struct_) => skip("struct", ctx.module.clone().join(&struct_.ident)),
            syn::Item::Enum(enum_) => skip("enum", ctx.module.clone().join(&enum_.ident)),
            syn::Item::Union(union_) => skip("union", ctx.module.clone().join(&union_.ident)),
            syn::Item::Trait(trait_) => skip("trait", ctx.module.clone().join(&trait_.ident)),
            syn::Item::TraitAlias(alias_) => skip("alias", ctx.module.clone().join(&alias_.ident)),
            syn::Item::Impl(impl_) => {
                info!("impl: {}", impl_.into_token_stream());
            }
            syn::Item::ForeignMod(_foreign_mod) => skip("foreign_mod", ctx.module.clone()),
            syn::Item::Verbatim(_verbatim_) => skip("verbatim", ctx.module.clone()),
            _ => skip("something else", ctx.module.clone()),
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
    Ok(db)
}

/// Parse a file into a syn::File.
pub fn parse_file(file: &FsPath) -> Result<syn::File, ResolveError> {
    trace!("parsing `{}`", file.display());

    let mut file = File::open(file)?;
    let mut source = String::new();
    file.read_to_string(&mut source)?;

    Ok(syn::parse_file(&source)?)
}

/// Find the path for a module.
/// TODO finish
pub fn find_path(parent_ctx: &ModuleCtx, item: &mut ModuleItem) -> Result<PathBuf, ResolveError> {
    let dir = parent_ctx.source_file.parent().ok_or(ResolveError::Root)?;
    let look_at = if let Some(path) = item.metadata.extract_attribute(&PATH) {
        let string = path.get_assigned_string().ok_or_else(ResolveError::MalformedPathAttribute(format!("{:?}", path)))?;
        if string.ends_with(".rs") {
            return Ok(dir.join(string));
        }
        string
    } else {
        format!("{}", item.name)
    };

    let rs = dir.join()

    if dir.

    panic!()
}

pub fn skip(kind: &str, path: AbsolutePath) {
    info!("skipping {} {:?}", kind, &path);
}

pub fn warn(span: &Span, cause: &dyn Display) {
    warn!("ignoring error [{:?}]: {}", span, cause);
}
