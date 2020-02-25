//! The Database data structure, which holds everything important about resolved code and is simple
//! to serialize to disk. Contains all the information needed for bindings generation.
//!
//! Important note: paths serve two purposes here. We identify items by their paths, but that's
//! not used for lookup when resolving code. All lookups go through the `bindings` tables in
//! `Namespace`. Every item has a binding corresponding to itself where it's introduced, but can of
//! course have other bindings. The `absolute_path` entry for each `Binding` tells you where
//! that item's stored in the `items` table of `Namespace` -- roughly,
//!
//! We don't model bindings past a single level of indirection. e.g., if you import a module `d`,
//! only that module is represented as being in your namespace; you have to look through it to find
//! its children.
//! We model enum namespaces as modules for simplicity.
//!
//! Invariant: If you look up a root scope,
//! you must have inserted and completed operating on that scope.

use crate::attributes::{HasMetadata, Visibility};
use crate::crates::CrateData;
use crate::identities::{CrateId, Identity};
use crate::items::{MacroItem, SymbolItem, TypeItem};
use crate::paths::Ident;
use crate::scopes::{Binding, NamespaceId, Priority, Scope};
use crate::Map;
use dashmap::mapref::entry::Entry as DEntry;
use dashmap::mapref::one::Ref as DRef;
use dashmap::mapref::one::RefMut as DRefMut;
use dashmap::DashMap;
use hashbrown::hash_map::Entry as HEntry;
use serde::{Deserialize, Serialize};

/// A database of everything. Crates should form a DAG. Crates cannot be modified once added.
/// TODO: check for changes against disk?
#[derive(Serialize, Deserialize)]
pub struct Db {
    /// Crates must be frozen at the start of the process
    crates: Map<CrateId, CrateData>,

    /// Types in the crate.
    types: Namespace<TypeItem>,

    /// Symbols in the crate (functions, statics, constants)
    symbols: Namespace<SymbolItem>,

    /// Macros in the crate.
    macros: Namespace<MacroItem>,

    /// All the scopes available.
    scopes: Namespace<Scope>,
}

impl Db {
    /// Create an empty database.
    pub fn new(crates: Map<CrateId, CrateData>) -> Db {
        Db {
            crates,
            types: Namespace::new(),
            symbols: Namespace::new(),
            macros: Namespace::new(),
            scopes: Namespace::new(),
        }
    }
}
/*
/// All items accessible via some crate. This is used to decide which items to bind.
/// Returns a sequence of relative paths to bindings in the current crate, and the paths to the
/// items they point to.
///
/// If there are multiple bindings to some target, the shortest / lexicographically first is selected.
/// Then, the whole list is sorted.
/// This helps ensures determinism of generated bindings between runs.
pub fn accessible_items<I: NamespaceLookup>(
    &self,
    crate_: &CrateId,
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
*/

/// A view into the database.
///
/// Taking multiple simultaneous refs into the `Db` can deadlock because of how `DashMap` works,
/// so we make it so that you need a `DbView` for all operations.
///
/// At the start of some set of operations (e.g. walking a crate) you should take out a `DbView`
/// and use *only that view* to modify the underlying database.
///
/// If you don't use any other DbViews while calling DbView methods, you can't deadlock;
/// the methods take `&mut`, so only one can be called at a time.
pub struct DbView<'a>(&'a Db);

impl<'a> DbView<'a> {
    /// Inspect an item. Takes a closure because it's easier than returning a Ref.
    pub fn get_item<I: NamespaceLookup, F, R>(&mut self, id: &Identity) -> Option<Ref<I>> {
        I::get_namespace(self.0).0.get(id)
    }

    /// Inspect an item. Takes a closure because it's easier than returning a Ref.
    pub fn get_item_mut<I: NamespaceLookup, F, R>(&mut self, id: &Identity) -> Option<RefMut<I>> {
        I::get_namespace(self.0).0.get_mut(id)
    }

    /// Add the root scope for a crate.
    pub fn add_root_scope(&mut self, crate_: CrateId, scope: Scope) -> Identity {
        let root_id = Identity::root(crate_);
        self.0.scopes.0.insert(root_id.clone(), scope);
        root_id
    }

    /// Insert an item, and add a binding for that item in the relevant module.
    pub fn add_item<I: NamespaceLookup>(
        &mut self,
        containing_scope: &Identity,
        id: Identity,
        item: I,
    ) -> Result<(), DatabaseError> {
        let visibility = item.metadata().visibility;
        debug_assert!(
            &item.metadata().name == &id.path[id.path.len() - 1],
            "item identity does not match name"
        );

        let name = id.path[id.path.len() - 1].clone();

        let namespace = I::get_namespace(self.0);

        match namespace.0.entry(id.clone()) {
            DEntry::Occupied(_) => {
                return Err(DatabaseError::ItemAlreadyPresent);
            }
            DEntry::Vacant(v) => {
                v.insert(item);
            }
        }

        self.add_binding::<I>(&containing_scope, name, id, visibility, Priority::Explicit)
    }

    /// Add a binding. Doesn't have to target something in this crate.
    pub fn add_binding<I: NamespaceLookup>(
        &self,
        containing_scope: &Identity,
        name: Ident,
        target: Identity,
        visibility: Visibility,
        priority: Priority,
    ) -> Result<(), DatabaseError> {
        let mut binding = Binding {
            identity: target,
            visibility,
            priority,
        };

        let mut scope = self
            .0
            .scopes
            .0
            .get_mut(&containing_scope)
            .ok_or(DatabaseError::NoSuchScope)?;
        let bindings = scope.get_bindings_mut::<I>();
        match bindings.entry(name) {
            HEntry::Occupied(mut old) => {
                let old = old.get_mut();

                if old.priority == Priority::Glob && binding.priority == Priority::Explicit {
                    // TODO: signal that this occurred?
                    std::mem::swap(old, &mut binding);
                    Ok(())
                } else {
                    Err(DatabaseError::BindingAlreadyPresent)
                }
            }
            HEntry::Vacant(v) => {
                v.insert(binding);
                Ok(())
            }
        }
    }

    pub fn crate_data(&self, crate_: &CrateId) -> &CrateData {
        // note: takes &self, doesn't need to lock a DashMap
        &self
            .0
            .crates
            .get(crate_)
            .expect("invariant violated: no such crate")
    }
}

pub type Ref<'a, I> = DRef<'a, Identity, I, ahash::RandomState>;
pub type RefMut<'a, I> = DRefMut<'a, Identity, I, ahash::RandomState>;

/// A global namespace.
#[derive(Serialize, Deserialize)]
pub struct Namespace<I>(DashMap<Identity, I, ahash::RandomState>);

impl<I: NamespaceLookup> Namespace<I> {
    fn new() -> Self {
        Namespace(DashMap::default())
    }
}

/// Generic helper.
pub trait NamespaceLookup: HasMetadata + Sized + 'static {
    fn namespace_id() -> NamespaceId;
    fn get_namespace(db: &Db) -> &Namespace<Self>;
}
impl NamespaceLookup for TypeItem {
    fn namespace_id() -> NamespaceId {
        NamespaceId::Type
    }
    fn get_namespace(db: &Db) -> &Namespace<Self> {
        &db.types
    }
}
impl NamespaceLookup for SymbolItem {
    fn namespace_id() -> NamespaceId {
        NamespaceId::Symbol
    }
    fn get_namespace(db: &Db) -> &Namespace<Self> {
        &db.symbols
    }
}
impl NamespaceLookup for MacroItem {
    fn namespace_id() -> NamespaceId {
        NamespaceId::Type
    }
    fn get_namespace(db: &Db) -> &Namespace<Self> {
        &db.macros
    }
}
impl NamespaceLookup for Scope {
    fn namespace_id() -> NamespaceId {
        NamespaceId::Scope
    }
    fn get_namespace(db: &Db) -> &Namespace<Self> {
        &db.scopes
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
        NoSuchScope {
            display("no such scope")
        }
    }
}
