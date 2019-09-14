//! Walk through a module, feeding data to syn and then a `resolver::Db`.
//! Expands macros as it goes.
use dashmap::DashMap;
use lazy_static::lazy_static;
use rayon::prelude::*;
use std::fmt::Display;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path as FsPath, PathBuf};
use syn::spanned::Spanned;
use tracing::{trace, trace_span, warn, info};
use std::sync::Arc;

use tendon_api::attributes::{Metadata, Span, Visibility};
use tendon_api::idents::Ident;
use tendon_api::items::{ModuleItem, SymbolItem, TypeItem, DeclarativeMacroItem, MacroItem};
use tendon_api::paths::{AbsoluteCrate, AbsolutePath, Path, UnresolvedPath};
use tendon_api::tokens::Tokens;

use crate::expand::{UnexpandedCursor, UnexpandedItem, UnexpandedModule};
use crate::lower::attributes::lower_metadata;
use crate::lower::items::lower_enum;
use crate::lower::items::lower_function_item;
use crate::lower::items::lower_struct;
use crate::lower::macros::lower_macro_rules;
use crate::lower::LowerError;
use crate::lower::{attributes::lower_visibility, imports::lower_use, modules::lower_module};
use crate::tools::CrateData;
use crate::{Db, Map};

// TODO: rearrange names so that they match >:(

lazy_static! {
    static ref MACRO_USE: Path = Path::fake("macro_use");
    static ref PATH: Path = Path::fake("path");
    static ref MACRO_RULES: Ident = "macro_rules".into();
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
    pub unexpanded: UnexpandedCursor<'a>,

    /// Unexpanded modules in this crate.
    pub crate_unexpanded_modules: &'a DashMap<AbsolutePath, UnexpandedModule>,

    /// The metadata for the current crate, including imports.
    pub crate_data: &'a CrateData,

    /// If we are currently expanding a macro, the macro we're expanding from.
    pub macro_invocation: Option<Arc<Span>>
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

/// Walk a whole crate in parallel, expanding macros, storing all resulting data in the central db.
pub fn walk_crate(crate_data: &mut CrateData, db: &Db) -> Result<(), WalkError> {
    trace!("walking {:?}", crate_data.crate_);

    let mut imports = ModuleScope::new();

    let root_path = AbsolutePath::root(crate_data.crate_.clone());

    let source_root = crate_data.entry.parent().unwrap().to_path_buf();

    let crate_unexpanded_modules = DashMap::default();
    let mut unexpanded = UnexpandedModule::new(source_root.clone());

    let mut ctx = WalkModuleCtx {
        source_file: crate_data.entry.clone(),
        module: root_path.clone(),
        db,
        scope: &mut imports,
        unexpanded: UnexpandedCursor::new(&mut unexpanded),
        crate_unexpanded_modules: &crate_unexpanded_modules,
        crate_data: &crate_data.clone(),
        macro_invocation: None
    };

    let file = parse_file(&crate_data.entry)?;

    for item in &file.items {
        let span = Span::new(
            ctx.macro_invocation.clone(),
            ctx.source_file.clone(), item.span()
        );

        if let syn::Item::ExternCrate(extern_crate) = item {
            let mut metadata = lower_metadata(
                &ctx,
                &extern_crate.vis,
                &extern_crate.attrs,
                extern_crate.span(),
            );

            let ident = Ident::from(&extern_crate.ident);
            let crate_ = unwrap_or_warn!(
                ctx.crate_data
                    .deps
                    .get(&ident)
                    .ok_or_else(|| WalkError::ExternCrateNotFound(ident.clone())),
                &span
            );

            if metadata.extract_attribute(&*MACRO_USE).is_some() {
                // this miiiight have weird ordering consequences... whatever
                ctx.unexpanded
                    .insert(UnexpandedItem::MacroUse(span.clone(), crate_.clone()));
            }

            if let Some((_, name)) = &extern_crate.rename {
                // add rename to crate namespace
                // note that this effect *only* occurs at the crate root: otherwise `extern crate`
                // just behaves like a `use`.
                crate_data.deps.insert(Ident::from(&name), crate_.clone());
            }
        }
    }

    // patch in data w/ modified extern crate names
    ctx.crate_data = crate_data;

    // Do everything in parallel
    walk_items_parallel(&mut ctx, &file.items)?;

    // get metadata for root crate entry
    let metadata = lower_metadata(&mut ctx, &syn::parse_quote!(pub), &file.attrs, file.span());
    let module = ModuleItem {
        name: Ident::from(root_path.crate_.name.to_string()),
        metadata,
    };

    // store results for root crate entry
    db.modules.insert(root_path.clone(), module)?;
    db.scopes.insert(root_path.clone(), imports)?;
    crate_unexpanded_modules.insert(root_path.clone(), unexpanded);

    let mut macros = Map::default();

    expand_module(db, &crate_unexpanded_modules, root_path.clone(), crate_data, &mut macros)?;

    Ok(())
}

/// Walk a set of items, spawning rayon tasks to walk submodules in parallel.
pub fn walk_items_parallel(ctx: &mut WalkModuleCtx, items: &[syn::Item]) -> Result<(), WalkError> {
    let walk_child = |mut mod_: ModuleItem,
                      source_file: PathBuf,
                      path: AbsolutePath,
                      items: &[syn::Item]|
     -> Result<(), WalkError> {
        let mut imports = ModuleScope::new();
        let mut unexpanded = UnexpandedModule::new(source_file.clone());

        {
            let mut ctx = WalkModuleCtx {
                source_file,
                crate_data: ctx.crate_data,
                module: ctx.module.clone().join(mod_.name.clone()),
                scope: &mut imports,
                unexpanded: UnexpandedCursor::new(&mut unexpanded),
                db: &ctx.db,
                crate_unexpanded_modules: &ctx.crate_unexpanded_modules,
                macro_invocation: None,
            };

            // Invoke children
            walk_items_parallel(&mut ctx, items)?;

            trace!("finished invoking children");
        }

        trace!("insert modules");
        ctx.db.modules.insert(path.clone(), mod_)?;
        trace!("insert scopes");
        ctx.db.scopes.insert(path.clone(), imports)?;
        trace!("insert unexpanded");
        ctx.crate_unexpanded_modules
            .insert(path.clone(), unexpanded);

        Ok(())
    };

    // find modules we can walk in parallel
    let parallel_mods: Vec<(ModuleItem, AbsolutePath)> = items
        .iter()
        .filter_map(|item| {
            if let syn::Item::Mod(mod_) = item {
                let lowered = lower_module(ctx, mod_);
                if mod_.content.is_none() {
                    return Some((lowered, ctx.module.clone().join(Ident::from(&mod_.ident))));
                } else if let Some((_, content)) = &mod_.content {
                    let path = ctx.module.clone().join(&lowered.name);
                    let result = walk_child(
                        lowered.clone(),
                        ctx.source_file.clone(),
                        path.clone(),
                        &content[..],
                    );

                    if let Err(err) = result {
                        // TODO use span
                        warn!("error parsing module {:?}: {}", path, err);
                    }
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
                let source_file = find_source_file(ctx, &mut mod_)?;
                let parsed = parse_file(&source_file)?;

                // fix up parsed module
                let Metadata {
                    visibility: _,
                    extra_attributes,
                    deprecated,
                    docs,
                    must_use: _,
                    span: _,
                } = lower_metadata(&ctx, &syn::parse_quote!(), &parsed.attrs, parsed.span());
                mod_.metadata
                    .extra_attributes
                    .extend(extra_attributes.into_iter());
                if let (None, Some(deprecated)) = (&mod_.metadata.deprecated, deprecated) {
                    mod_.metadata.deprecated = Some(deprecated);
                }
                if let (None, Some(docs)) = (&mod_.metadata.docs, docs) {
                    mod_.metadata.docs = Some(docs);
                }

                // do the rest of the walking
                walk_child(mod_, source_file, path.clone(), &parsed.items[..])
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
            Visibility::Pub => $ctx.scope.pub_imports.insert(
                $item.name.clone(),
                $ctx.module.clone().join($item.name.clone()).into(),
            ),
            Visibility::NonPub => $ctx.scope.imports.insert(
                $item.name.clone(),
                $ctx.module.clone().join($item.name.clone()).into(),
            ),
        }
    };
}

/// Parse a set of items into a database.
pub fn walk_items(ctx: &mut WalkModuleCtx, items: &[syn::Item]) -> Result<(), WalkError> {
    let _span = trace_span!("walk_items", path = tracing::field::debug(&ctx.module));

    trace!("walking {:?}", ctx.module);

    for item in items {
        let span = Span::new(ctx.macro_invocation.clone(), ctx.source_file.clone(), item.span());

        use quote::ToTokens;

        // TODO: enforce not using `?` here?
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
                        .insert(UnexpandedItem::TypeMacro(span, Tokens::from(fn_))),
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
                        .insert(UnexpandedItem::TypeMacro(span, Tokens::from(struct_))),
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
                        .insert(UnexpandedItem::TypeMacro(span, Tokens::from(enum_))),
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
                ctx.unexpanded.insert(UnexpandedItem::MacroInvocation(
                    span,
                    Tokens::from(macro_rules_),
                ));
            }
            syn::Item::Use(use_) => {
                // note: modifies ctx directly!
                lower_use(ctx, use_);
            }
            syn::Item::ExternCrate(extern_crate) => {
                if ctx.module.path.is_empty() {
                    // crate root `extern crate`s have special semantics
                    // and have already been handled in walk_crate
                    continue;
                }
                let mut metadata = lower_metadata(
                    ctx,
                    &extern_crate.vis,
                    &extern_crate.attrs,
                    extern_crate.span(),
                );

                let ident = Ident::from(&extern_crate.ident);
                let crate_ = unwrap_or_warn!(
                    ctx.crate_data
                        .deps
                        .get(&ident)
                        .ok_or_else(|| WalkError::ExternCrateNotFound(ident.clone())),
                    &span
                );
                let imports = match metadata.visibility {
                    Visibility::Pub => &mut ctx.scope.pub_imports,
                    Visibility::NonPub => &mut ctx.scope.imports,
                };
                let no_path: &[&str] = &[];
                imports.insert(ident, AbsolutePath::new(crate_.clone(), no_path).into());
            }
            _ => skip("something else", ctx.module.clone()),
        }
    }

    trace!("done walking items");

    // TODO: check edition?
    Ok(())
}

/// Walk a modules, expanding all encountered macros as we go.
/// This should be run *after* a full initial parsing run. It will operate serially.
fn expand_module(db: &Db,
                 unexpanded_modules: &DashMap<AbsolutePath, UnexpandedModule>,
                 path: AbsolutePath,
                 crate_data: &CrateData,
                 macros: &mut Map<Ident, DeclarativeMacroItem>,
) -> Result<(), WalkError> {
    let unexpanded = unexpanded_modules.remove(&path);
    let mut unexpanded = if let Some((_, unexpanded)) = unexpanded {
        unexpanded
    } else {
        return Ok(());
    };

    // this won't cause errors because this function is never called more than once for a particular module
    db.scopes.take_modify(&path, |scope| {
        let mut ctx = WalkModuleCtx {
            source_file: unexpanded.source_file.clone(),
            module: path.clone(),
            db,
            scope,
            unexpanded: UnexpandedCursor::new(&mut unexpanded),
            // note: this is only used to insert submodules for macro expansion;
            // the fact that the current module is missing won't be a problem
            crate_unexpanded_modules: unexpanded_modules,
            crate_data,
            macro_invocation: None
        };
        ctx.unexpanded.reset();

        while let Some(item) = ctx.unexpanded.pop() {
            let span = item.span().clone();

            // suppress errors here, don't return them
            let result = (|| -> Result<(), WalkError> {
                match item {
                    UnexpandedItem::MacroUse(span, crate_) => {
                        for path in db.macros.iter_crate(&crate_) {
                            unwrap_or_warn!(db.macros.inspect(&path, |macro_| {
                            if let MacroItem::Declarative(macro_) = macro_ {
                                macros.insert(macro_.name.clone(), macro_.clone());
                            }
                            Ok(())
                        }), &span);
                        }
                    }
                    UnexpandedItem::UnexpandedModule { span, name, macro_use } => {
                        if macro_use {
                            expand_module(db, unexpanded_modules, path.clone().join(name), crate_data, macros)?;
                        } else {
                            let mut macros = macros.clone();
                            expand_module(db, unexpanded_modules, path.clone().join(name), crate_data, &mut macros)?;
                        }
                    },
                    UnexpandedItem::MacroInvocation(span, inv) => {
                        let inv = inv.parse::<syn::ItemMacro>().unwrap();

                        // TODO: attributes?

                        let target = UnresolvedPath::from(&inv.mac.path);

                        if let Some(ident) = target.get_ident() {
                            if ident == &*MACRO_RULES {
                                let def = lower_macro_rules(&ctx, &inv)?;
                                if def.macro_export {
                                    let path = AbsolutePath::new(ctx.crate_data.crate_.clone(), &[def.name.clone()]);
                                    ctx.db.macros.insert(path, MacroItem::Declarative(def.clone()));
                                }
                                info!("found macro {}", def.name);
                                macros.insert(def.name.clone(), def);
                                return Ok(());
                            } else if let Some(macro_) = macros.get(ident) {
                                let result = crate::expand::apply_once(macro_, inv.mac.tokens.clone())?;
                                let parsed: syn::File = syn::parse2(result)?;
                                info!("applying macro {}! in {:?}", ident, path);

                                let result = walk_items_parallel(&mut ctx, &parsed.items[..]);
                                // make sure we expand anything we discovered next
                                ctx.unexpanded.reset();

                                return Ok(());
                            }
                        }

                        warn!("failed to resolve macro: {:?}", target);
                        // TODO: path lookups
                    }
                    UnexpandedItem::TypeMacro(span, inv) => {
                        trace!("skipping type-position macro at {:?}, unimplemented", span)
                    }
                    UnexpandedItem::AttributeMacro(span, inv) => {
                        trace!("skipping attribute macro at {:?}, unimplemented", span)
                    }
                    UnexpandedItem::DeriveMacro(span, inv) => {
                        trace!("skipping #[derive] at {:?}, unimplemented", span)
                    }
                }
                Ok(())
            })();

            if let Err(err) = result {
                warn(&err, &span)
            }
        }
        Ok(())
    })?;

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

    let mut root = parent_ctx.crate_data.entry.parent().unwrap().to_owned();

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
        ExternCrateNotFound(ident: Ident) {
            display("can't find extern crate: {}", ident)
        }
    }
}

#[cfg(test)]
macro_rules! test_ctx {
    ($ctx:pat) => {
        let source_file = std::path::PathBuf::from("fake_file.rs");
        let module = tendon_api::paths::AbsolutePath::root(tendon_api::paths::AbsoluteCrate::new(
            "fake_crate",
            "0.0.1",
        ));
        let db = crate::Db::new();
        let mut scope = crate::walker::ModuleScope::new();
        let mut unexpanded = crate::expand::UnexpandedModule::new(source_file.clone());
        let crate_unexpanded_modules = dashmap::DashMap::default();
        let crate_data = crate::tools::CrateData::fake();

        let $ctx = crate::walker::WalkModuleCtx {
            source_file,
            module,
            db: &db,
            scope: &mut scope,
            unexpanded: crate::expand::UnexpandedCursor::new(&mut unexpanded),
            crate_unexpanded_modules: &crate_unexpanded_modules,
            crate_data: &crate_data,
            macro_invocation: None
        };
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walk_no_fs() {
        spoor::init();

        let db = Db::new();
        let source_file = PathBuf::from("/fake/fake_file.rs");
        let crate_ = AbsoluteCrate::new("fake_crate", "0.1.0");
        let mut scope = ModuleScope::new();
        let mut unexpanded = UnexpandedModule::new(source_file.clone());
        let crate_unexpanded_modules = DashMap::default();
        let mut ctx = WalkModuleCtx {
            module: AbsolutePath::root(crate_.clone()),
            source_file,
            db: &db,
            scope: &mut scope,
            unexpanded: UnexpandedCursor::new(&mut unexpanded),
            crate_unexpanded_modules: &crate_unexpanded_modules,
            crate_data: &CrateData::fake(),
            macro_invocation: None
        };

        let fake: syn::File = syn::parse_quote! {
            extern crate bees as thing;

            // note: non-pub fns will be ignored
            pub fn f(y: i32) -> i32 {}

            enum X {}
            enum Y {}

            #[derive(Debug)]
            struct TestStruct {
                x: X, y: Y
            }
        };

        walk_items(&mut ctx, &fake.items).unwrap();

        assert!(db
            .symbols
            .contains(&AbsolutePath::new(crate_.clone(), &["f"])));
        assert!(db
            .types
            .contains(&AbsolutePath::new(crate_.clone(), &["TestStruct"])));
        assert!(db
            .types
            .contains(&AbsolutePath::new(crate_.clone(), &["X"])));
        assert!(db
            .types
            .contains(&AbsolutePath::new(crate_.clone(), &["Y"])));
    }
}
