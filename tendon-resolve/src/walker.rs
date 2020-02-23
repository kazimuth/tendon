//! Walk through a module, feeding data to syn and then a `resolver::Db`.
//!
//! This code is serial but multiple crates can be read into the same Db at once.
//!
//! references:
//! https://rust-lang.github.io/rustc-guide/macro-expansion.html
//! https://rust-lang.github.io/rustc-guide/name-resolution.html
//! https://doc.rust-lang.org/nightly/edition-guide/rust-2018/macros/macro-changes.html

use dashmap::DashMap;
use lazy_static::lazy_static;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use syn::spanned::Spanned;
use tracing::{trace, trace_span, warn};

use tendon_api::attributes::{Metadata, Span, Visibility};
use tendon_api::idents::Ident;
use tendon_api::items::{DeclarativeMacroItem, MacroItem, ModuleItem, SymbolItem, TypeItem};
use tendon_api::paths::{AbsoluteCrate, AbsolutePath, Path, UnresolvedPath};
use tendon_api::tokens::Tokens;

use crate::lower::attributes::lower_metadata;
use crate::tools::CrateData;

/*
use crate::lower::items::lower_enum;
use crate::lower::items::lower_function_item;
use crate::lower::items::lower_struct;
use crate::lower::macros::lower_macro_rules;
use crate::lower::{imports::lower_use, modules::lower_module};
*/
use crate::lower::LowerError;
use crate::{Db, Map};

mod textual_scope;

/// The first phase: parse files, find imports, macro_interp macros.
/// As items cease to have anything to do with macros they are dumped in the Db; after macro
/// expansion order no longer matters.
struct WalkExpandPhase<'a> {
    /// A Db containing resolved definitions for all dependencies.
    /// During this phase, we insert `ModuleItems` and `MacroItems` into this database.
    /// The only items that go in here are `#[macro_export]`-marked macros.
    db: &'a Db,

    /// Declarative macro items inserted into the crate prelude when marked with `#[macro_export]`.
    /// Also, macros brought in with `#[macro_use] extern crate` are placed here, but NOT in the
    /// crate root.
    ///
    /// This information is discarded after parsing this crate.
    ///
    /// TODO: add std / core macros here?
    /// TODO: figure out what happens when we override those
    macro_prelude: Map<Ident, DeclarativeMacroItem>,

    /// The contents of modules we've discovered.
    modules: Map<AbsolutePath, ParsedModule>,
}

struct ParsedModule {
    /// Where this module is (in the file system and the crate namespace).
    loc: LocationMetadata,
    /// Imports to this module
    scope: ModuleScope,
}

pub(crate) struct LocationMetadata {
    pub(crate) source_file: PathBuf,
    pub(crate) macro_invocation: Option<Arc<Span>>,
    pub(crate) crate_data: Arc<CrateData>,
    pub(crate) module_path: AbsolutePath,
}

// A scope.
// Each scope currently corresponds to a module; that might change if we end up having to handle
// impl's in function scopes.
#[derive(Default)]
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

/// Walk a whole crate, expanding macros, storing all resulting data in the central db.
/// This operates serially but multiple threads can operate on the same db in parallel.
pub fn walk_crate(crate_data: &mut CrateData, db: &Db) -> Result<(), WalkError> {
    trace!("walking {:?}", crate_data.crate_);

    /*
    let mut expand_phase = ExpandPhase {
        db,
        modules: Default::default(),
        global_macros_used: vec![]
    };

    // prep roots
    let root_path = AbsolutePath::root(crate_data.crate_.clone());
    let source_root = crate_data.entry.parent().unwrap().to_path_buf();

    let file = parse_file(&crate_data.entry)?;
    */

    // TODO extern crate extra effects at crate root:
    // - global alias
    // - #[macro_use] injects into all modules

    /*
    let root_module = ParsedModule {
        loc: LocationMetadata {
            source_file: &crate_data.entry.clone(),
            macro_invocation: None
        },
        items: vec![],
        scope: Default::default()
    };

    // prep modules
    let mut imports = ModuleScope::new();

    // the root context
    let mut ctx = WalkModuleCtx {
        source_file: crate_data.entry.clone(),
        module: root_path.clone(),
        db,
        scope: &mut imports,
        unexpanded: UnexpandedCursor::new(&mut unexpanded),
        crate_unexpanded_modules: &crate_unexpanded_modules,
        crate_data: &crate_data.clone(),
        macro_invocation: None,
    };

    // special case: walk the crate root looking for extern crates.
    // they behave special here for legacy reasons.
    for item in &file.items {
        let span = Span::new(
            ctx.macro_invocation.clone(),
            ctx.source_file.clone(),
            item.span(),
        );

        let result = (|| -> Result<(), WalkError> {
            if let syn::Item::ExternCrate(extern_crate) = item {
            }
            Ok(())
        })();

        if let Err(err) = result {
            warn(err, &span);
        }
    }

    // patch in data w/ modified extern crate names
    ctx.crate_data = crate_data;

    // get metadata for root crate entry
    let metadata = lower_metadata(&mut ctx, &syn::parse_quote!(pub), &file.attrs, file.span())?;
    let module = ModuleItem {
        name: Ident::from(root_path.crate_.name.to_string()),
        metadata,
    };

    // store results for root crate entry
    db.modules.insert(root_path.clone(), module)?;
    crate_unexpanded_modules.insert(root_path.clone(), unexpanded);

    //
    walk_items(&mut ctx, &file.items)?;

    let mut local_sequential_macros = Map::default();

    expand_module(
        db,
        &crate_unexpanded_modules,
        root_path.clone(),
        crate_data,
        &mut local_sequential_macros,
    )?;
    */

    Ok(())
}

macro_rules! skip_non_pub {
    ($item:expr) => {
        match &$item.vis {
            syn::Visibility::Public(_) => (),
            _ => return Ok(()),
        }
    };
}
/*

/// Lower an item.
fn lower_item(loc: &LocationMetadata, item: &syn::Item) -> Result<(), WalkError> {
    let span = Span::new(
        loc.macro_invocation.clone(),
        loc.source_file.clone(),
        item.span(),
    );
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
                    ctx.db.symbols.insert(
                        ctx.module.clone().join(&fn_.name),
                        SymbolItem::Function(fn_),
                    )?
                }
                Err(LowerError::TypePositionMacro) => ctx
                    .unexpanded
                    .insert(UnexpandedItem::TypeMacro(span, Tokens::from(fn_))),
                err => {
                    err?;
                }
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
                    ctx.db.types.insert(
                        ctx.module.clone().join(&struct_.name),
                        TypeItem::Struct(struct_),
                    )?
                }
                Err(LowerError::TypePositionMacro) => ctx
                    .unexpanded
                    .insert(UnexpandedItem::TypeMacro(span, Tokens::from(struct_))),
                err => {
                    err?;
                }
            }
        }
        syn::Item::Enum(enum_) => {
            let result = lower_enum(ctx, enum_);
            match result {
                Ok(enum_) => {
                    add_to_scope!(ctx, enum_);
                    ctx.db
                        .types
                        .insert(ctx.module.clone().join(&enum_.name), TypeItem::Enum(enum_))?;
                }
                Err(LowerError::TypePositionMacro) => ctx
                    .unexpanded
                    .insert(UnexpandedItem::TypeMacro(span, Tokens::from(enum_))),
                err => {
                    err?;
                }
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
                return Ok(());
            }
            let metadata = lower_metadata(
                ctx,
                &extern_crate.vis,
                &extern_crate.attrs,
                extern_crate.span(),
            )?;

            let ident = Ident::from(&extern_crate.ident);
            let crate_ = ctx
                .crate_data
                .deps
                .get(&ident)
                .ok_or_else(|| WalkError::ExternCrateNotFound(ident.clone()))?;
            let imports = match metadata.visibility {
                Visibility::Pub => &mut ctx.scope.pub_imports,
                Visibility::NonPub => &mut ctx.scope.imports,
            };
            let no_path: &[&str] = &[];
            imports.insert(ident, AbsolutePath::new(crate_.clone(), no_path).into());
        }
        _ => skip("something else", ctx.module.clone()),
    }
    Ok(())
}


/// A module with macros unexpanded.
/// We throw all macro-related stuff here when we're walking freshly-parsed modules.
/// It's not possible to eagerly macro_interp macros because they rely on name resolution to work, and we
/// can't do name resolution (afaict) until after we've lowered most modules already.
/// This is ordered because order affects macro name resolution.
#[derive(Debug)]
pub struct UnexpandedModule {
    items: Vec<UnexpandedItem>,
    pub source_file: PathBuf,
}
impl UnexpandedModule {
    /// Create an empty unexpanded module.
    pub fn new(source_file: PathBuf) -> Self {
        UnexpandedModule {
            items: vec![],
            source_file,
        }
    }
}

#[derive(Debug)]
/// An item during the macro_interp
pub enum UnexpandedItem {
    /// A macro invocation in item position,
    UnresolvedMacroInvocation(Span, Tokens),

    /// Some item that contains a macro in type position.
    TypeMacro(Span, Tokens),

    /// Something with an attribute macro applied.
    AttributeMacro(Span, Tokens),

    /// Something with a derive macro applied.
    /// Note: the item itself should already be stored in the main `Db`, and doesn't need to be
    /// re-added.
    DeriveMacro(Span, Tokens),
    /// A sub module that has yet to be expanded.
    UnexpandedModule {
        span: Span,
        name: Ident,
        macro_use: bool,
    },
    /// An import with #[macro_use].
    MacroUse(Span, AbsoluteCrate),
}

impl UnexpandedItem {
    pub fn span(&self) -> &Span {
        match self {
            UnexpandedItem::MacroInvocation(span, _) => span,
            UnexpandedItem::TypeMacro(span, _) => span,
            UnexpandedItem::AttributeMacro(span, _) => span,
            UnexpandedItem::DeriveMacro(span, _) => span,
            UnexpandedItem::UnexpandedModule { span, .. } => span,
            UnexpandedItem::MacroUse(span, _) => span,
        }
    }
}

/// A cursor examining an unexpanded module.
pub struct UnexpandedCursor<'a> {
    pub module: &'a mut UnexpandedModule,
    idx: usize,
}
impl<'a> UnexpandedCursor<'a> {
    /// Crate a cursor into a module.
    pub fn new(module: &'a mut UnexpandedModule) -> UnexpandedCursor<'a> {
        let idx = module.items.len();
        UnexpandedCursor { module, idx }
    }
    /// Insert something into the module.
    pub fn insert(&mut self, item: UnexpandedItem) {
        self.module.items.insert(self.idx, item);
        self.idx += 1;
    }
    /// Reset to the front of the target module.
    pub fn reset(&mut self) {
        self.idx = 0;
    }
    /// Pop the item at the cursor position.
    pub fn pop(&mut self) -> Option<UnexpandedItem> {
        if self.module.items.len() <= self.idx {
            None
        } else {
            Some(self.module.items.remove(self.idx))
        }
    }
}
*/

/// Parse a file into a syn::File.
fn parse_file(file: &FsPath) -> Result<syn::File, WalkError> {
    trace!("parsing `{}`", file.display());

    let mut file = File::open(file)?;
    let mut source = String::new();
    file.read_to_string(&mut source)?;

    Ok(syn::parse_file(&source)?)
}

/// Find the path for a module.
fn find_source_file(
    expand_phase: &WalkExpandPhase,
    parent: &LocationMetadata,
    item: &mut ModuleItem,
) -> Result<PathBuf, WalkError> {
    let look_at = if let Some(path) = item.metadata.extract_attribute(&PATH) {
        let string = path
            .get_assigned_string()
            .ok_or_else(|| WalkError::MalformedPathAttribute(format!("{:?}", path)))?;
        if string.ends_with(".rs") {
            // TODO are there more places we should check?
            let dir = parent.source_file.parent().ok_or(WalkError::Root)?;
            return Ok(dir.join(string));
        }
        string
    } else {
        format!("{}", item.name)
    };

    let mut root_normal = parent.crate_data.entry.parent().unwrap().to_owned();
    for entry in &parent.module_path.path {
        root_normal.push(entry.to_string());
    }

    let root_renamed = parent.source_file.parent().unwrap().to_owned();

    for root in [root_normal, root_renamed].iter() {
        let to_try = [
            root.join(format!("{}.rs", look_at)),
            root.join(look_at.clone()).join("mod.rs"),
        ];
        for to_try in to_try.iter() {
            if let Ok(metadata) = fs::metadata(to_try) {
                if metadata.is_file() {
                    return Ok(to_try.clone());
                }
            }
        }
    }

    Err(WalkError::ModuleNotFound)
}

/*
fn handle_root_extern_crate(loc: &LocationMetadata, extern_crate: &syn::ItemExternCrate) {
    let mut metadata = lower_metadata(
        loc,
        &extern_crate.vis,
        &extern_crate.attrs,
        extern_crate.span(),
    )?;

    let ident = Ident::from(&extern_crate.ident);
    let crate_ = ctx
        .crate_data
        .deps
        .get(&ident)
        .ok_or_else(|| WalkError::ExternCrateNotFound(ident.clone()))?;

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
*/

pub fn skip(kind: &str, path: AbsolutePath) {
    trace!("skipping {} {:?}", kind, &path);
}

pub fn warn(cause: impl Into<WalkError>, span: &Span) {
    let cause = cause.into();
    if let WalkError::Lower(LowerError::CfgdOut) = cause {
        // can just suppress this
        return;
    }
    warn!("[{:?}]: suppressing error: {}", span, cause);
}

lazy_static! {
    static ref MACRO_USE: Path = Path::fake("macro_use");
    static ref PATH: Path = Path::fake("path");
    static ref MACRO_RULES: Ident = "macro_rules".into();
    pub(crate) static ref TEST_LOCATION_METADATA: LocationMetadata = LocationMetadata {
        source_file: "fake_file.rs".into(),
        macro_invocation: None,
        crate_data: Arc::new(CrateData::fake()),
        module_path: AbsolutePath {
            crate_: AbsoluteCrate::new("fake_crate", "0.0.0"),
            path: vec![]
        }
    };
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
        NonPub {
            display("skipping non-pub item (will never be accessible)")
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

/*
/// Walk a module.
fn parse_and_expand_module(
    phase: &ExpandPhase,
    path: AbsolutePath,
    local_sequential_macros: &mut Map<Ident, DeclarativeMacroItem>,
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
            macro_invocation: None,
        };
        ctx.unexpanded.reset();

        while let Some(item) = ctx.unexpanded.pop() {
            let span = item.span().clone();

            // suppress errors here, don't return them
            let result = (|| -> Result<(), WalkError> {
                match item {
                    UnexpandedItem::MacroUse(_, crate_) => {
                        for path in db.macros.iter_crate(&crate_) {
                            db.macros.inspect(&path, |macro_| {
                                if let MacroItem::Declarative(macro_) = macro_ {
                                    local_sequential_macros
                                        .insert(macro_.name.clone(), macro_.clone());
                                }
                                Ok(())
                            })?;
                        }
                    }
                    UnexpandedItem::UnexpandedModule {
                        name, macro_use, ..
                    } => {
                        if macro_use {
                            expand_module(
                                db,
                                unexpanded_modules,
                                path.clone().join(name),
                                crate_data,
                                local_sequential_macros,
                            )?;
                        } else {
                            // TODO do we need to pass these in at all?
                            let mut local_sequential_macros = local_sequential_macros.clone();

                            expand_module(
                                db,
                                unexpanded_modules,
                                path.clone().join(name),
                                crate_data,
                                &mut local_sequential_macros,
                            )?;
                        }
                    }
                    UnexpandedItem::MacroInvocation(span, inv) => {
                        let inv = inv.parse::<syn::ItemMacro>().unwrap();

                        // TODO: attributes?

                        let target = UnresolvedPath::from(&inv.mac.path);

                        if let Some(ident) = target.get_ident() {
                            if ident == &*MACRO_RULES {
                                let def = lower_macro_rules(&ctx, &inv)?;
                                if def.macro_export {
                                    let path = AbsolutePath::new(
                                        ctx.crate_data.crate_.clone(),
                                        &[def.name.clone()],
                                    );
                                    ctx.db
                                        .macros
                                        .insert(path, MacroItem::Declarative(def.clone()))?;
                                }
                                trace!("found macro {}", def.name);
                                local_sequential_macros.insert(def.name.clone(), def);
                                return Ok(());
                            } else if let Some(macro_) = local_sequential_macros.get(ident) {
                                let result =
                                    crate::macro_interp::apply_once(macro_, inv.mac.tokens.clone())?;
                                let parsed: syn::File = syn::parse2(result)?;
                                trace!("applying macro {}! in {:?}", ident, path);

                                let result = walk_items(&mut ctx, &parsed.items[..]);
                                // make sure we macro_interp anything we discovered next
                                ctx.unexpanded.reset();

                                result?;

                                return Ok(());
                            }
                        }

                        warn!("[{:?}]: failed to resolve macro: {:?}", span, target);

                        // TODO: path lookups
                    }
                    UnexpandedItem::TypeMacro(span, _) => {
                        trace!("skipping type-position macro at {:?}, unimplemented", span)
                    }
                    UnexpandedItem::AttributeMacro(span, _) => {
                        trace!("skipping attribute macro at {:?}, unimplemented", span)
                    }
                    UnexpandedItem::DeriveMacro(span, _) => {
                        trace!("skipping #[derive] at {:?}, unimplemented", span)
                    }
                }
                Ok(())
            })();

            if let Err(err) = result {
                warn(err, &span)
            }
        }
        Ok(())
    })?;

    Ok(())
}
*/

/*
/// Context for lowering items in an individual module.
pub struct WalkModuleCtx<'a> {
    /// A Db containing resolved definitions for all dependencies.
    pub db: &'a Db,

    /// The location of this module's containing file in the filesystem.
    pub source_file: PathBuf,

    /// The module path.
    pub module: AbsolutePath,

    /// The scope for this module.
    pub scope: &'a mut ModuleScope,

    /// All items in this module that need to be macro-expanded.
    pub unexpanded: UnexpandedCursor<'a>,

    /// The metadata for the current crate, including imports.
    pub crate_data: &'a CrateData,

    /// If we are currently expanding a macro, the macro we're expanding from.
    pub macro_invocation: Option<Arc<Span>>,
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


fn merge_metadata(target: &mut Metadata, from: Metadata) {
    let Metadata {
        extra_attributes,
        deprecated,
        docs,
        ..
    } = from;

    target.metadata
        .extra_attributes
        .extend(extra_attributes.into_iter());
    if let (None, Some(deprecated)) = (&lowered.metadata.deprecated, deprecated) {
        target.metadata.deprecated = Some(deprecated);
    }
    if let (None, Some(docs)) = (&lowered.metadata.docs, docs) {
        target.metadata.docs = Some(docs);
    }
}

/// Parse a set of items into a database.
pub fn walk_items(ctx: &mut WalkModuleCtx, items: &[syn::Item]) -> Result<(), WalkError> {
    let _span = trace_span!("walk_items", path = tracing::field::debug(&ctx.module));

    trace!("walking {:?}", ctx.module);

    for item in items {
        let result = walk_item(ctx, item);

        if let Err(err) = result {
            let span = Span::new(
                ctx.macro_invocation.clone(),
                ctx.source_file.clone(),
                item.span(),
            );
            warn(err, &span)
        }
    }

    trace!("done walking items");

    Ok(())
}

fn walk_mod(ctx: &mut WalkModuleCtx, mod_: &syn::ItemMod) -> Result<(), WalkError> {
    // lower the ModuleItem (i.e. its attributes and stuff, not its contents)
    let mut lowered = lower_module(ctx, mod_)?;

    // the path of the submodule
    let path = ctx.module.clone().join(&lowered.name);

    // borrowck juggling...
    let items: Vec<syn::Item>;
    let content: &[syn::Item];
    let source_file;

    if let Some((_, inline_content)) = &mod_.content {
        // the module conveniently provides its contents right here!
        content = &inline_content[..];
        source_file = ctx.source_file.clone();
    } else {
        // we gotta go find the module's source file.
        source_file = find_source_file(ctx, &mut lowered)?;
        // parse it.
        let parsed = parse_file(&source_file)?;

        // now that we've loaded the source file, let's attach its metadata to the original declaration.
        merge_metadata(&mut lowered.metadata,
            lower_metadata(&ctx, &syn::parse_quote!(), &parsed.attrs, parsed.span())?);

        // prep for recursion.
        let syn::File { items: items_, .. } = parsed;
        items = items_;
        content = &items[..];
    }

    let mut imports = ModuleScope::new();
    let mut unexpanded = UnexpandedModule::new(source_file.clone());

    {
        let mut ctx = WalkModuleCtx {
            source_file,
            crate_data: ctx.crate_data,
            module: ctx.module.clone().join(lowered.name.clone()),
            scope: &mut imports,
            unexpanded: UnexpandedCursor::new(&mut unexpanded),
            db: &ctx.db,
            crate_unexpanded_modules: &ctx.crate_unexpanded_modules,
            macro_invocation: None,
        };

        // Invoke children
        walk_items(&mut ctx, content)?;

        trace!("finished invoking children");
    }

    trace!("insert modules");
    ctx.db.modules.insert(path.clone(), lowered)?;
    trace!("insert scopes");
    ctx.db.scopes.insert(path.clone(), imports)?;
    trace!("insert unexpanded");
    ctx.crate_unexpanded_modules
        .insert(path.clone(), unexpanded);

    Ok(())
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


/*
/

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
        let mut unexpanded = crate::macro_interp::UnexpandedModule::new(source_file.clone());
        let crate_unexpanded_modules = dashmap::DashMap::default();
        let crate_data = crate::tools::CrateData::fake();

        let $ctx = crate::walker::WalkModuleCtx {
            source_file,
            module,
            db: &db,
            scope: &mut scope,
            unexpanded: crate::macro_interp::UnexpandedCursor::new(&mut unexpanded),
            crate_unexpanded_modules: &crate_unexpanded_modules,
            crate_data: &crate_data,
            macro_invocation: None,
        };
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use tendon_api::paths::AbsoluteCrate;

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
            macro_invocation: None,
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
*/
*/
