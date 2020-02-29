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
use crate::identities::{CrateId, Identity, TEST_CRATE_A, TEST_CRATE_B, TEST_CRATE_C};
use crate::items::{MacroItem, SymbolItem, TypeItem};
use crate::paths::{Ident, UnresolvedPath};
use crate::scopes::{Binding, NamespaceId, Priority, Scope};
use crate::Map;
use dashmap::mapref::entry::Entry as DEntry;
use dashmap::mapref::one::Ref as DRef;
use dashmap::mapref::one::RefMut as DRefMut;
use dashmap::DashMap;
use hashbrown::hash_map::Entry as HEntry;
use serde::{Deserialize, Serialize};

lazy_static! {
    pub static ref ROOT_SCOPE_NAME: Ident = "{root}".into();
}

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

    /// Create a DB view. You should only create one view per thread, to prevent deadlocks due to
    /// DashMap.
    pub fn view_once_per_thread_i_promise(&self) -> DbView {
        DbView(self)
    }

    #[allow(unused)]
    /// Creates a `Db` for tests.
    pub(crate) fn fake_db() -> Db {
        let crate_a = CrateData::fake(TEST_CRATE_A.clone());
        let crate_b = CrateData::fake(TEST_CRATE_B.clone());
        let crate_c = CrateData::fake(TEST_CRATE_C.clone());

        let mut crates = Map::default();
        crates.insert(TEST_CRATE_A.clone(), crate_a);
        crates.insert(TEST_CRATE_B.clone(), crate_b);
        crates.insert(TEST_CRATE_C.clone(), crate_c);

        Db::new(crates)
    }
}

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
    /// Get an item.
    pub fn get_item<I: NamespaceLookup, F, R>(&mut self, id: &Identity) -> Option<Ref<I>> {
        I::get_namespace(self.0).0.get(id)
    }

    /// Get an item mutably.
    pub fn get_item_mut<I: NamespaceLookup, F, R>(&mut self, id: &Identity) -> Option<RefMut<I>> {
        I::get_namespace(self.0).0.get_mut(id)
    }

    /// Add the root scope for a crate.
    pub fn add_root_scope(&mut self, crate_: CrateId, scope: Scope) -> Identity {
        assert!(&scope.metadata.name == &*ROOT_SCOPE_NAME);
        let root_id = Identity::root(&crate_);
        self.0.scopes.0.insert(root_id.clone(), scope);
        root_id
    }

    /// Insert an item, and add a binding for that item in the relevant module.
    /// Returns the identity for the item (
    pub fn add_item<I: NamespaceLookup>(
        &mut self,
        containing_scope: &Identity,
        item: I,
    ) -> Result<Identity, DatabaseError> {
        let visibility = item.metadata().visibility.clone();
        let name = item.metadata().name.clone();
        let identity = containing_scope.clone_join(name.clone());

        let namespace = I::get_namespace(self.0);

        match namespace.0.entry(identity.clone()) {
            DEntry::Occupied(_) => {
                return Err(DatabaseError::ItemAlreadyPresent);
            }
            DEntry::Vacant(v) => {
                v.insert(item);
            }
        }

        self.add_binding::<I>(
            &containing_scope,
            name,
            identity.clone(),
            visibility,
            Priority::Explicit,
        )?;

        Ok(identity)
    }

    /// Add a binding. Doesn't have to target something in this crate.
    pub fn add_binding<I: NamespaceLookup>(
        &mut self,
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
                    // overrides previous.
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
///
/// Invariant: if `namespace[I] == item`, `I[-1] == item.metadata().name`, UNLESS
/// `I == []`, i.e. it is a crate root.
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
        BindingNotFound {
            display("item not found")
        }
        NoSuchScope {
            display("no such scope")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attributes::Metadata;

    #[test]
    fn insert_and_query() {
        let db = Db::fake_db();
        let mut view = db.view_once_per_thread_i_promise();

        let root = view.add_root_scope(
            TEST_CRATE_A.clone(),
            Scope::new(Metadata::fake(&*ROOT_SCOPE_NAME), true),
        );

        let some_module = view
            .add_item(&root, Scope::new(Metadata::fake("some_module"), true))
            .unwrap();

        let next_module = view
            .add_item(
                &some_module,
                Scope::new(Metadata::fake("next_module"), true),
            )
            .unwrap();
    }
}
