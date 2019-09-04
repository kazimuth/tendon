//! Walk through a module, feeding data to syn and then a `resolver::Db`.
//! Expands macros as it goes.
//!
//! TODO: move around stuff from "resolver" to "walker", make this its own top-level module
use dashmap::DashMap;
use lazy_static::lazy_static;
use rayon::prelude::*;
use std::fmt::Display;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path as FsPath, PathBuf};
use syn::spanned::Spanned;
use tracing::{trace, trace_span, warn};

use transgress_api::attributes::{Metadata, Span, Visibility};
use transgress_api::idents::Ident;
use transgress_api::items::{ModuleItem, SymbolItem, TypeItem};
use transgress_api::paths::{AbsoluteCrate, AbsolutePath, Path};
use transgress_api::tokens::Tokens;

use crate::lower::attributes::lower_metadata;
use crate::lower::items::lower_enum;
use crate::lower::items::lower_function_item;
use crate::lower::items::lower_struct;
use crate::lower::LowerError;
use crate::lower::{attributes::lower_visibility, imports::lower_use, modules::lower_module};
use crate::tools::CrateData;
use crate::{Db, Map};

// TODO: ignore all non-`pub` items

lazy_static! {
    static ref MACRO_USE: Path = Path::fake("macro_use");
    static ref PATH: Path = Path::fake("path");
}

macro_rules! unwrap_or_warn {
    ($result:expr, $span:expr) => {
        match $result {
            Ok(result) => result,
            Err(err) => {
                warn(&err, $span);
                continue;
            }
        }
    };
}

/// Context for lowering items in an individual module.
pub struct WalkModuleCtx<'a> {
    /// The location of this module's containing file in the filesystem.
    pub source_file: PathBuf,
    /// The module path.
    pub module: AbsolutePath,

    /// A Db containing resolved definitions for all dependencies.
    pub db: &'a Db,

    /// The scope for this module.
    pub scope: &'a mut ModuleScope,

    /// All items in this module that need to be macro-expanded.
    pub unexpanded: &'a mut UnexpandedModule,

    /// Unexpanded modules in this crate.
    pub crate_unexpanded_modules: &'a DashMap<AbsolutePath, UnexpandedModule>,

    /// The source root (i.e. directory containing root lib.rs file) of this crate
    pub source_root: &'a PathBuf,
}

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
        let db = crate::Db::new();
        let mut scope = crate::walker::ModuleScope::new();
        let mut unexpanded = crate::walker::UnexpandedModule::new();
        let crate_unexpanded_modules = dashmap::DashMap::default();
        let source_root = std::path::PathBuf::from("fake_src/");

        let $ctx = WalkModuleCtx {
            source_file,
            module,
            db: &db,
            scope: &mut scope,
            unexpanded: &mut unexpanded,
            crate_unexpanded_modules: &crate_unexpanded_modules,
            source_root: &source_root,
        };
    };
}

// A scope.
// Each scope currently corresponds to a module; that might change if we end up having to handle
// impl's in function scopes.
pub struct ModuleScope {
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

impl ModuleScope {
    /// Create a new set of imports
    pub fn new() -> ModuleScope {
        ModuleScope {
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
    UnexpandedModule { name: Ident, macro_use: bool },
}

/// Walk a whole crate in parallel, storing all resulting data in the central db.
pub fn walk_crate(
    crate_: AbsoluteCrate,
    data: &CrateData,
    db: &Db,
) -> Result<DashMap<AbsolutePath, UnexpandedModule>, WalkError> {
    trace!("walking {:?}", crate_);

    let mut imports = ModuleScope::new();
    let mut unexpanded = UnexpandedModule::new();
    let crate_unexpanded_modules = DashMap::default();

    let path = AbsolutePath {
        crate_,
        path: vec![],
    };

    let source_root = data.entry.parent().unwrap().to_path_buf();

    let mut ctx = WalkModuleCtx {
        source_file: data.entry.clone(),
        source_root: &source_root,
        module: path.clone(),
        db,
        scope: &mut imports,
        unexpanded: &mut unexpanded,
        crate_unexpanded_modules: &crate_unexpanded_modules,
    };

    let file = parse_file(&data.entry)?;

    // Do everything in parallel
    walk_items_parallel(&mut ctx, &file.items)?;

    // get metadata for root crate entry
    let metadata = lower_metadata(&mut ctx, &syn::parse_quote!(pub), &file.attrs, file.span());
    let module = ModuleItem {
        name: Ident::from(path.crate_.name.to_string()),
        metadata,
    };

    // store results for root crate entry
    db.modules.insert(path.clone(), module)?;
    db.scopes.insert(path.clone(), imports)?;
    crate_unexpanded_modules.insert(path.clone(), unexpanded);

    Ok(crate_unexpanded_modules)
}

/// Walk a set of items, spawning rayon tasks to walk submodules in parallel.
pub fn walk_items_parallel(ctx: &mut WalkModuleCtx, items: &[syn::Item]) -> Result<(), WalkError> {
    // find modules we can walk in parallel
    let parallel_mods: Vec<(ModuleItem, AbsolutePath)> = items
        .iter()
        .filter_map(|item| {
            if let syn::Item::Mod(mod_) = item {
                // have to walk inline mods serially
                if mod_.content.is_none() {
                    return Some((
                        lower_module(ctx, mod_),
                        ctx.module.clone().join(Ident::from(&mod_.ident)),
                    ));
                }
            }
            None
        })
        .collect();

    trace!(
        "{:?} children: {:#?}",
        ctx.module,
        parallel_mods
            .iter()
            .map(|(_, path)| path)
            .collect::<Vec<_>>()
    );

    parallel_mods
        .into_par_iter()
        .for_each(|(mut mod_, path): (ModuleItem, AbsolutePath)| {
            // -- PARALLEL --

            // poor man's try:
            let result = (|| -> Result<(), WalkError> {
                let mut imports = ModuleScope::new();
                let mut unexpanded = UnexpandedModule::new();

                let source_file = find_source_file(ctx, &mut mod_)?;
                let parsed = parse_file(&source_file)?;

                {
                    let mut ctx = WalkModuleCtx {
                        source_file,
                        source_root: ctx.source_root,
                        module: ctx.module.clone().join(mod_.name.clone()),

                        scope: &mut imports,
                        unexpanded: &mut unexpanded,

                        db: &ctx.db,
                        crate_unexpanded_modules: &ctx.crate_unexpanded_modules,
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
                    } = lower_metadata(
                        &mut ctx,
                        &syn::parse_quote!(),
                        &parsed.attrs,
                        parsed.span(),
                    );

                    mod_.metadata
                        .extra_attributes
                        .extend(extra_attributes.into_iter());
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
                ctx.crate_unexpanded_modules
                    .insert(path.clone(), unexpanded);

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

macro_rules! skip_non_pub {
    ($item:expr) => {
        match &$item.vis {
            syn::Visibility::Public(_) => (),
            _ => continue,
        }
    };
}
macro_rules! add_to_scope {
    ($ctx:ident, $item:ident) => {
        match &$item.metadata.visibility {
            Visibility::Pub => {
                $ctx.scope.pub_imports.insert($item.name.clone(), $ctx.module.clone().join($item.name.clone()).into())
            }
            Visibility::NonPub => {
                $ctx.scope.imports.insert($item.name.clone(), $ctx.module.clone().join($item.name.clone()).into())
            }
        }
    };
}

/// Parse a set of items into a database.
pub fn walk_items(ctx: &mut WalkModuleCtx, items: &[syn::Item]) -> Result<(), WalkError> {
    let _span = trace_span!("walk_items", path = tracing::field::debug(&ctx.module));

    trace!("walking {:?}", ctx.module);

    for item in items {
        let span = Span::from_syn(ctx.source_file.clone(), item.span());

        match item {
            syn::Item::Static(static_) => {
                skip_non_pub!(static_);
                skip("static", ctx.module.clone().join(&static_.ident))
                // TODO: add to scope when implemented
            }
            syn::Item::Const(const_) => {
                skip_non_pub!(const_);
                skip("const", ctx.module.clone().join(&const_.ident))
                // TODO: add to scope when implemented
            }
            syn::Item::Fn(fn_) => {
                skip_non_pub!(fn_);
                let result = lower_function_item(ctx, fn_);
                match result {
                    Ok(fn_) => {
                        add_to_scope!(ctx, fn_);
                        unwrap_or_warn!(
                            ctx.db.symbols.insert(
                                ctx.module.clone().join(&fn_.name),
                                SymbolItem::Function(fn_)
                            ),
                            &span
                        );
                    }
                    Err(LowerError::TypePositionMacro) => ctx
                        .unexpanded
                        .0
                        .push(UnexpandedItem::TypeMacro(span, Tokens::from(fn_))),
                    Err(other) => warn(&other, &span),
                }
            }
            // note: we don't skip non-pub items for the rest of this, since we need to know about
            // all types for send + sync determination
            syn::Item::Type(type_) => {
                skip("type", ctx.module.clone().join(&type_.ident))
                // TODO: add to scope when implemented
            }
            syn::Item::Struct(struct_) => {
                let result = lower_struct(ctx, struct_);
                match result {
                    Ok(struct_) => {
                        add_to_scope!(ctx, struct_);
                        unwrap_or_warn!(
                            ctx.db.types.insert(
                                ctx.module.clone().join(&struct_.name),
                                TypeItem::Struct(struct_)
                            ),
                            &span
                        );
                    }
                    Err(LowerError::TypePositionMacro) => ctx
                        .unexpanded
                        .0
                        .push(UnexpandedItem::TypeMacro(span, Tokens::from(struct_))),
                    Err(other) => warn(&other, &span),
                }
            }
            syn::Item::Enum(enum_) => {
                let result = lower_enum(ctx, enum_);
                match result {
                    Ok(enum_) => {
                        add_to_scope!(ctx, enum_);
                        unwrap_or_warn!(
                            ctx.db.types.insert(
                                ctx.module.clone().join(&enum_.name),
                                TypeItem::Enum(enum_)
                            ),
                            &span
                        )
                    }
                    Err(LowerError::TypePositionMacro) => ctx
                        .unexpanded
                        .0
                        .push(UnexpandedItem::TypeMacro(span, Tokens::from(enum_))),
                    Err(other) => warn(&other, &span),
                }
            }
            syn::Item::Union(union_) => {
                skip("union", ctx.module.clone().join(&union_.ident))
                // TODO: add to scope when implemented
            }
            syn::Item::Trait(trait_) => {
                skip("trait", ctx.module.clone().join(&trait_.ident))
                // TODO: add to scope when implemented
            }
            syn::Item::TraitAlias(alias_) => {
                skip("trait alias", ctx.module.clone().join(&alias_.ident))
                // TODO: add to scope when implemented
            }
            syn::Item::Impl(_impl_) => skip("impl", ctx.module.clone()),
            syn::Item::ForeignMod(_foreign_mod) => skip("foreign_mod", ctx.module.clone()),
            syn::Item::Verbatim(_verbatim_) => skip("verbatim", ctx.module.clone()),
            syn::Item::Macro(macro_rules_) => {
                ctx.unexpanded.0.push(UnexpandedItem::MacroInvocation(
                    span,
                    Tokens::from(macro_rules_),
                ));
            }
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
            syn::Item::ExternCrate(_extern_crate) => skip("extern crate", ctx.module.clone()),
            _ => skip("something else", ctx.module.clone()),
        }
    }

    trace!("done walking items");

    // TODO: check edition?
    Ok(())
}

/// Parse a file into a syn::File.
pub fn parse_file(file: &FsPath) -> Result<syn::File, WalkError> {
    trace!("parsing `{}`", file.display());

    let mut file = File::open(file)?;
    let mut source = String::new();
    file.read_to_string(&mut source)?;

    Ok(syn::parse_file(&source)?)
}

/// Find the path for a module.
pub fn find_source_file(
    parent_ctx: &WalkModuleCtx,
    item: &mut ModuleItem,
) -> Result<PathBuf, WalkError> {
    let look_at = if let Some(path) = item.metadata.extract_attribute(&PATH) {
        let string = path
            .get_assigned_string()
            .ok_or_else(|| WalkError::MalformedPathAttribute(format!("{:?}", path)))?;
        if string.ends_with(".rs") {
            // TODO are there more places we should check?
            let dir = parent_ctx.source_file.parent().ok_or(WalkError::Root)?;
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
        root.join(look_at).join("mod.rs"),
    ];

    for to_try in to_try.iter() {
        if let Ok(metadata) = fs::metadata(to_try) {
            if metadata.is_file() {
                return Ok(to_try.clone());
            }
        }
    }

    Err(WalkError::ModuleNotFound)
}

pub fn skip(kind: &str, path: AbsolutePath) {
    trace!("skipping {} {:?}", kind, &path);
}

pub fn warn(cause: &dyn Display, span: &Span) {
    warn!("ignoring error [{:?}]: {}", span, cause);
}

quick_error! {
    #[derive(Debug)]
    pub enum WalkError {
        Io(err: std::io::Error) {
            from()
            cause(err)
            description(err.description())
            display("io error during walking: {}", err)
        }
        Parse(err: syn::Error) {
            from()
            cause(err)
            description(err.description())
            display("parse error during walking: {}", err)
        }
        Resolve(err: crate::resolver::ResolveError) {
            from()
            cause(err)
            description(err.description())
            display("name resolution error during walking: {}", err)
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
        Other {
            display("other error")
        }
    }
}
