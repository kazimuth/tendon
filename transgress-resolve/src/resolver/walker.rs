//! Walk through a module, feeding data to syn and then a `resolver::Db`.
//! Expands macros as it goes.
use lazy_static::lazy_static;
use quote::ToTokens;
use std::fmt::Display;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path as FsPath, PathBuf};
use syn::spanned::Spanned;
use tracing::{info, trace, warn, trace_span};
use rayon::prelude::*;
use dashmap::DashMap;

use transgress_api::attributes::{Span, Visibility, Metadata};
use transgress_api::items::{ModuleItem};
use transgress_api::paths::{AbsolutePath, Path, AbsoluteCrate};
use transgress_api::idents::Ident;

use super::{ModuleCtx, ModuleImports, ResolveError};
use crate::{
    lower::{
        attributes::lower_visibility,
        imports::lower_use,
        modules::lower_module,
    },
};
use crate::resolver::{UnexpandedModule, Db, CrateData};
use crate::lower::attributes::lower_metadata;

lazy_static! {
    static ref MACRO_USE: Path = Path::fake("macro_use");
    static ref PATH: Path = Path::fake("path");
}

macro_rules! unwrap_or_warn {
    ($result:expr, $span:expr) => {
        match $result {
            Ok(result) => result,
            Err(err) => {
                warn($span, &err);
                continue;
            }
        }
    };
}

/// Walk a whole crate in parallel, storing all resulting data in the central db.
pub fn walk_crate(crate_: AbsoluteCrate, data: &CrateData, db: &Db) -> Result<DashMap<AbsolutePath, UnexpandedModule>, ResolveError> {
    trace!("walking {:?}", crate_);

    let mut imports = ModuleImports::new();
    let mut unexpanded = UnexpandedModule::new();
    let crate_unexpanded_modules = DashMap::default();

    let path = AbsolutePath { crate_, path: vec![] };

    let source_root = data.entry.parent().unwrap().to_path_buf();

    let mut ctx = ModuleCtx {
        source_file: data.entry.clone(),
        source_root: &source_root,
        module: path.clone(),
        db,
        scope: &mut imports,
        unexpanded: &mut unexpanded,
        crate_unexpanded_modules: &crate_unexpanded_modules
    };

    let file = parse_file(&data.entry)?;

    // Do everything in parallel
    walk_items_parallel(&mut ctx, &file.items)?;

    // get metadata for root crate entry
    let metadata = lower_metadata(&mut ctx, &syn::parse_quote!(pub), &file.attrs, file.span());
    let module = ModuleItem {
        name: Ident::from(path.crate_.name.to_string()),
        metadata
    };

    // store results for root crate entry
    db.modules.insert(path.clone(), module)?;
    db.scopes.insert(path.clone(), imports)?;
    crate_unexpanded_modules.insert(path.clone(), unexpanded);

    Ok(crate_unexpanded_modules)
}

/// Walk a set of items, spawning rayon tasks to walk submodules in parallel.
pub fn walk_items_parallel(ctx: &mut ModuleCtx, items: &[syn::Item]) -> Result<(), ResolveError> {

    // find modules we can walk in parallel
    let parallel_mods: Vec<(ModuleItem, AbsolutePath)> = items.iter().filter_map(|item| {
        if let syn::Item::Mod(mod_) = item {
            // have to walk inline mods serially
            if mod_.content.is_none() {
                return Some((lower_module(ctx, mod_), ctx.module.clone().join(Ident::from(&mod_.ident))));
            }
        }
        None
    }).collect();

    trace!("{:?} children: {:#?}", ctx.module, parallel_mods.iter().map(|(_, path)| path).collect::<Vec<_>>());

    parallel_mods.into_iter().for_each(|(mut mod_, path): (ModuleItem, AbsolutePath)| {
        // -- PARALLEL --

        // poor man's try:
        let result = (|| -> Result<(), ResolveError> {
            let mut imports = ModuleImports::new();
            let mut unexpanded = UnexpandedModule::new();

            let source_file = find_source_file(ctx, &mut mod_)?;
            let parsed = parse_file(&source_file)?;

            {
                let mut ctx = ModuleCtx {
                    source_file,
                    source_root: ctx.source_root,
                    module: ctx.module.clone().join(mod_.name.clone()),

                    scope: &mut imports,
                    unexpanded: &mut unexpanded,

                    db: &ctx.db,
                    crate_unexpanded_modules: &ctx.crate_unexpanded_modules
                };

                // Invoke children
                walk_items_parallel(&mut ctx, &parsed.items)?;

                trace!("finished invoking children");

                // Fix up metadata
                let Metadata {
                    visibility: _,
                    extra_attributes,
                    deprecated,
                    docs,
                    must_use: _,
                    span: _,
                } = lower_metadata(&mut ctx, &syn::parse_quote!(), &parsed.attrs, parsed.span());

                mod_.metadata.extra_attributes.extend(extra_attributes.into_iter());
                if let (None, Some(deprecated)) = (&mod_.metadata.deprecated, deprecated) {
                    mod_.metadata.deprecated = Some(deprecated);
                }
                if let (None, Some(docs)) = (&mod_.metadata.docs, docs) {
                    mod_.metadata.docs = Some(docs);
                }
            }

            trace!("insert modules");
            ctx.db.modules.insert(path.clone(), mod_)?;
            trace!("insert scopes");
            ctx.db.scopes.insert(path.clone(), imports)?;
            trace!("insert unexpanded");
            ctx.crate_unexpanded_modules.insert(path.clone(), unexpanded);

            trace!("finished insertion");
            Ok(())
        })();

        if let Err(err) = result {
            // TODO use span
            warn!("error parsing module {:?}: {}", path, err);
        }
    });

    // now do the actual lowering for this crate
    // note: the fact that we save this for after all submodules are done parsing means the current parsed file
    // hangs out in memory a long time
    // might be able to fix that somehow...
    let result = walk_items(ctx, items);

    trace!("parallel walk items complete");

    result
}

/// Parse a set of items into a database.
pub fn walk_items(ctx: &mut ModuleCtx, items: &[syn::Item]) -> Result<(), ResolveError> {
    let _span = trace_span!("walk_items", path = tracing::field::debug(&ctx.module));

    trace!("walking {:?}", ctx.module);

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
            syn::Item::Macro(_macro) => skip("macro", ctx.module.clone()),
            syn::Item::Use(use_) => {
                if lower_visibility(&use_.vis) == Visibility::Pub {
                    lower_use(
                        &use_,
                        &mut ctx.scope.pub_glob_imports,
                        &mut ctx.scope.pub_imports,
                    );
                } else {
                    lower_use(&use_, &mut ctx.scope.glob_imports, &mut ctx.scope.imports);
                }
            }
            syn::Item::ExternCrate(extern_crate) => skip("extern crate", ctx.module.clone()),
            _ => skip("something else", ctx.module.clone()),
        }
    }

    trace!("done walking items");

    // TODO: check edition?
    Ok(())
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
pub fn find_source_file(parent_ctx: &ModuleCtx, item: &mut ModuleItem) -> Result<PathBuf, ResolveError> {
    let look_at = if let Some(path) = item.metadata.extract_attribute(&PATH) {
        let string = path.get_assigned_string().ok_or_else(|| ResolveError::MalformedPathAttribute(format!("{:?}", path)))?;
        if string.ends_with(".rs") {
            // TODO are there more places we should check?
            let dir = parent_ctx.source_file.parent().ok_or(ResolveError::Root)?;
            return Ok(dir.join(string));
        }
        string
    } else {
        format!("{}", item.name)
    };

    let mut root = parent_ctx.source_root.clone();

    for entry in &parent_ctx.module.path {
        root.push(entry.to_string());
    }

    let to_try = [
        root.join(format!("{}.rs", look_at)),
        root.join(look_at).join("mod.rs")
    ];

    for to_try in to_try.iter() {
        if let Ok(metadata) = fs::metadata(to_try) {
            if metadata.is_file() {
                return Ok(to_try.clone())
            }
        }
    }

    Err(ResolveError::ModuleNotFound(parent_ctx.module.clone().join(&item.name)))
}

pub fn skip(kind: &str, path: AbsolutePath) {
    info!("skipping {} {:?}", kind, &path);
}

pub fn warn(span: &Span, cause: &dyn Display) {
    warn!("ignoring error [{:?}]: {}", span, cause);
}
