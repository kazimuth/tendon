//! The Database data structure, which holds everything important about resolved code and is simple
//! to serialize to disk. Contains all the information needed for bindings generation.
//!
//! Important note: paths serve two purposes here. We identify items by their paths, but that's
//! not used for lookup when resolving code. All lookups go through the `bindings` tables in
//! `Namespace`. Every item has a binding corresponding to itself where it's introduced, but can of
//! course have other bindings. The `absolute_path` entry for each `Binding` tells you where
//! that item's stored in the `items` table of `Namespace` -- roughly,

use crate::attributes::{HasMetadata, Visibility};
use crate::crates::CrateData;
use crate::items::{MacroItem, ModuleItem, SymbolItem, TypeItem};
use crate::paths::{AbsoluteCrate, AbsolutePath, Identity};
use crate::Map;
use dashmap::DashMap;
use hashbrown::hash_map::Entry;
use serde::{Deserialize, Serialize};

/// A database of everything. Crates should form a DAG. Crates cannot be modified once added.
/// TODO: check for changes against disk?
#[derive(Serialize, Deserialize)]
pub struct Db {
    // TODO: this isn't serializeable w/ fxhash, make a PR for that
    // TODO: this could maybe be optimized w/ some sort of append-only representation
    crates: DashMap<AbsoluteCrate, CrateDb>,
}

impl Db {
    /// Create an empty database.
    pub fn new() -> Db {
        Db {
            crates: DashMap::default(),
        }
    }

    /// Add a crate to the database. The crate should be finished resolving.
    pub fn add_crate(&self, crate_db: CrateDb) {
        self.crates
            .insert(crate_db.crate_data.crate_.clone(), crate_db);
    }

    /// All items accessible via some crate. This is used to decide which items to bind.
    /// Returns a sequence of relative paths to bindings in the current crate, and the paths to the
    /// items they point to.
    ///
    /// If there are multiple bindings to some target, the shortest / lexicographically first is selected.
    /// Then, the whole list is sorted.
    /// This helps ensures determinism of generated bindings between runs.
    pub fn accessible_items<I: NamespaceLookup>(
        &self,
        crate_: &AbsoluteCrate,
    ) -> Vec<(AbsolutePath, Identity)> {
        let crate_db = self.crates.get(crate_).expect("no such crate");
        let namespace = I::get_crate_namespace(&crate_db);
        let mut result: Map<&Identity, &AbsolutePath> = Map::default();

        for (path, binding) in &namespace.bindings {
            let is_containing_module_public = path
                .parent()
                .map(|p| crate_db.is_module_externally_visible(&p))
                .unwrap_or(true);
            if binding.visibility == Visibility::Pub && is_containing_module_public {
                match result.entry(&binding.identity) {
                    Entry::Vacant(v) => {
                        v.insert(path);
                    }
                    Entry::Occupied(mut o) => {
                        let should_replace = {
                            let cur_access_path = o.get_mut();

                            let new_shorter = path.path.len() < cur_access_path.path.len();
                            let new_same_and_lexicographically_earlier =
                                path.path.len() == cur_access_path.path.len() && path < cur_access_path;

                            new_shorter || new_same_and_lexicographically_earlier
                        };

                        if should_replace {
                            o.insert(path);
                        }
                    }
                }
            }
        }

        let mut result: Vec<(AbsolutePath, Identity)> = result
            .into_iter()
            .map(|(abs, rel)| (rel.clone(), abs.clone()))
            .collect();
        result.sort();
        result
    }

    /// Inspect an item. Takes a closure because it's easier than returning a Ref.
    /// TODO can this be optimized? 2 hash function lookups...
    pub fn inspect_item<I: NamespaceLookup, F, R>(&self, path: &Identity, op: F) -> R
    where
        F: FnOnce(Option<&I>) -> R,
    {
        let crate_ = self.crates.get(&path.0.crate_);
        let item = crate_
            .as_ref()
            .and_then(|crate_db| crate_db.get_item(&path));

        op(item)
    }

    /// Inspect an item. Takes a closure because it's easier than returning a Ref.
    /// TODO can this be optimized? 2 hash function lookups...
    pub fn inspect_binding<I: NamespaceLookup, F, R>(&self, path: &AbsolutePath, op: F) -> R
    where
        F: FnOnce(Option<&Binding>) -> R,
    {
        let crate_ = self.crates.get(&path.crate_);
        let item = crate_
            .as_ref()
            .and_then(|crate_db| crate_db.get_binding::<I>(&path));

        op(item)
    }
}

/// A database of everything found within a crate.
#[derive(Serialize, Deserialize)]
pub struct CrateDb {
    /// The crate's metadata.
    pub crate_data: CrateData,

    /// Types in the crate.
    types: CrateNamespace<TypeItem>,

    /// Symbols in the crate (functions, statics, constants)
    symbols: CrateNamespace<SymbolItem>,

    /// Macros in the crate.
    macros: CrateNamespace<MacroItem>,

    /// `mod` items, store metadata + privacy information, incl. the root module.
    modules: CrateNamespace<ModuleItem>,
}

impl CrateDb {
    /// Create a new database.
    pub fn new(crate_data: CrateData) -> CrateDb {
        CrateDb {
            types: CrateNamespace::new(crate_data.crate_.clone()),
            symbols: CrateNamespace::new(crate_data.crate_.clone()),
            macros: CrateNamespace::new(crate_data.crate_.clone()),
            modules: CrateNamespace::new(crate_data.crate_.clone()),
            crate_data,
        }
    }

    pub fn get_item<I: NamespaceLookup>(&self, id: &Identity) -> Option<&I> {
        self.assert_in_crate(&id.0.crate_);

        I::get_crate_namespace(self).items.get(id)
    }

    pub fn get_item_mut<I: NamespaceLookup>(&mut self, id: &Identity) -> Option<&mut I> {
        self.assert_in_crate(&id.0.crate_);

        I::get_crate_namespace_mut(self).items.get_mut(id)
    }

    pub fn get_binding<I: NamespaceLookup>(&self, path: &AbsolutePath) -> Option<&Binding> {
        self.assert_in_crate(&path.crate_);

        I::get_crate_namespace(self).bindings.get(path)
    }

    /// Check if a module is externally visible.
    pub fn is_module_externally_visible(&self, mod_: &AbsolutePath) -> bool {
        self.assert_in_crate(&mod_.crate_);

        let mut cur_check = AbsolutePath::root(mod_.crate_.clone());
        for entry in &mod_.path {
            cur_check.path.push(entry.clone()); // don't check root

            if self
                .get_binding::<ModuleItem>(&cur_check)
                .expect("checking missing module?")
                .visibility
                == Visibility::NonPub
            {
                return false;
            }
        }
        true
    }

    fn assert_in_crate(&self, crate_: &AbsoluteCrate) {
        debug_assert_eq!(&self.crate_data.crate_, crate_);
    }
}

/// A namespace within a crate, for holding some particular type of item during resolution.
/// `I` isn't constrained by `NamespaceLookup` for testing purposes but in effect it is.
#[derive(Serialize, Deserialize)]
pub struct CrateNamespace<I> {
    /// The AbsoluteCrate for this namespace.
    /// (stored redundantly for convenience.)
    crate_: AbsoluteCrate,

    /// True values, stored by the paths where they're defined. Note that this
    /// isn't used for binding lookups, just for storing actual values.
    items: Map<Identity, I>,

    /// Bindings.
    ///
    /// Note that every item has a binding added corresponding to itself within its module.
    ///
    /// Note also that these are collapsed: if you have `a reexports b reexports c`, this should map `a`
    /// to `c`, skipping `b`. This property is easy enough to ensure by construction.
    bindings: Map<AbsolutePath, Binding>,
}

impl<I: NamespaceLookup> CrateNamespace<I> {
    /// Create a namespace within a crate.
    fn new(crate_: AbsoluteCrate) -> Self {
        CrateNamespace {
            crate_,
            items: Map::default(),
            bindings: Map::default(),
        }
    }

    /// Insert an item, and add a binding for that item in the relevant module.
    pub fn add_item(&mut self, id: Identity, item: I) -> Result<(), DatabaseError> {
        let visibility = item.metadata().visibility;

        match self.items.entry(id.clone()) {
            Entry::Occupied(_) => return Err(DatabaseError::ItemAlreadyPresent),
            Entry::Vacant(v) => v.insert(item),
        };

        self.add_binding(id.0.clone(), id, visibility, Priority::Explicit)
    }

    /// Add a binding. Doesn't have to target something in this crate.
    pub fn add_binding(
        &mut self,
        path: AbsolutePath,
        identity: Identity,
        visibility: Visibility,
        priority: Priority,
    ) -> Result<(), DatabaseError> {
        let mut binding = Binding {
            identity,
            visibility,
            priority,
        };

        match self.bindings.entry(path) {
            Entry::Occupied(mut old) => {
                let old = old.get_mut();

                if old.priority == Priority::Glob && binding.priority == Priority::Explicit {
                    // TODO: signal that this occurred?
                    std::mem::swap(old, &mut binding);
                    Ok(())
                } else {
                    Err(DatabaseError::BindingAlreadyPresent)
                }
            }
            Entry::Vacant(v) => {
                v.insert(binding);
                Ok(())
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    Glob,
    Explicit,
}

/// A name binding. (Nothing to do with the idea of "language bindings".)
#[derive(Serialize, Deserialize, Clone)]
pub struct Binding {
    /// The final target this binding points to. If this points to another binding
    /// (e.g. a reexport), that chain is followed.
    pub identity: Identity,

    /// The visibility of the binding (NOT the item), in the scope of its module.
    pub visibility: Visibility,

    /// If the binding is through a glob or explicit.
    pub priority: Priority,
}

/// A namespace.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Namespace {
    Type,
    Symbol,
    Macro,
    Module,
}

/// Generic code helper.
pub trait NamespaceLookup: HasMetadata + Sized + 'static {
    fn namespace() -> Namespace;
    fn get_crate_namespace_mut(crate_db: &mut CrateDb) -> &mut CrateNamespace<Self>;
    fn get_crate_namespace(crate_db: &CrateDb) -> &CrateNamespace<Self>;
}
impl NamespaceLookup for TypeItem {
    fn namespace() -> Namespace {
        Namespace::Type
    }
    fn get_crate_namespace_mut(crate_db: &mut CrateDb) -> &mut CrateNamespace<Self> {
        &mut crate_db.types
    }
    fn get_crate_namespace(crate_db: &CrateDb) -> &CrateNamespace<Self> {
        &crate_db.types
    }
}
impl NamespaceLookup for SymbolItem {
    fn namespace() -> Namespace {
        Namespace::Symbol
    }
    fn get_crate_namespace_mut(crate_db: &mut CrateDb) -> &mut CrateNamespace<Self> {
        &mut crate_db.symbols
    }
    fn get_crate_namespace(crate_db: &CrateDb) -> &CrateNamespace<Self> {
        &crate_db.symbols
    }
}
impl NamespaceLookup for MacroItem {
    fn namespace() -> Namespace {
        Namespace::Macro
    }
    fn get_crate_namespace_mut(crate_db: &mut CrateDb) -> &mut CrateNamespace<Self> {
        &mut crate_db.macros
    }
    fn get_crate_namespace(crate_db: &CrateDb) -> &CrateNamespace<Self> {
        &crate_db.macros
    }
}
impl NamespaceLookup for ModuleItem {
    fn namespace() -> Namespace {
        Namespace::Module
    }
    fn get_crate_namespace_mut(crate_db: &mut CrateDb) -> &mut CrateNamespace<Self> {
        &mut crate_db.modules
    }
    fn get_crate_namespace(crate_db: &CrateDb) -> &CrateNamespace<Self> {
        &crate_db.modules
    }
}

quick_error::quick_error! {
    #[derive(Debug, Clone, Copy)]
    pub enum DatabaseError {
        ItemAlreadyPresent {
            display("item has already been added?")
        }
        BindingAlreadyPresent {
            display("item is already reexported?")
        }
        ItemNotFound {
            display("item not found")
        }
    }
}
