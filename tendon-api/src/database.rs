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
use crate::paths::Ident;
use crate::scopes::{Binding, NamespaceId, Prelude, Priority, Scope};
use crate::Map;
use hashbrown::hash_map::Entry as HEntry;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

lazy_static! {
    pub static ref ROOT_SCOPE_NAME: Ident = "{root}".into();
}

mod serializers;

/// A database of everything -- all declarations tendon cares about in a crate + its dependencies.
///
/// We operate on this in parallel, but we only need lightweight synchronization because of
/// the structure of the problem. We build up crates in parallel, and add them here once we've
/// completely finished parsing and lowering their contents.
///
/// Crates form a DAG that we know ahead of time, so we can just add OnceCells to store them
/// at initialization. There's never a need to take a lock on a whole map since it's entirely pre-
/// allocated.
///
/// The whole database is `serde::Serialize` as well, to save it you can just write it to a file. No need
/// for complex serialization infrastructure.
#[derive(Serialize, Deserialize)]
pub struct Db {
    /// Crate metadata must be frozen at the start of the process.
    crate_data: Map<CrateId, CrateData>,

    /// Lowered crate data.
    #[serde(serialize_with = "serializers::serialize_map_once_cell")]
    #[serde(deserialize_with = "serializers::deserialize_map_once_cell")]
    crates: Map<CrateId, OnceCell<Crate>>,
}

impl Db {
    /// Create an empty database.
    pub fn new(crate_data: Map<CrateId, CrateData>) -> Db {
        let crates = crate_data
            .keys()
            .map(|k| (k.clone(), OnceCell::new()))
            .collect();
        Db { crates, crate_data }
    }

    /// Creates a `Db` for tests.
    pub fn fake_db() -> Db {
        let crate_a = CrateData::fake(TEST_CRATE_A.clone());
        let crate_b = CrateData::fake(TEST_CRATE_B.clone());
        let crate_c = CrateData::fake(TEST_CRATE_C.clone());

        let mut crates = Map::default();
        crates.insert(TEST_CRATE_A.clone(), crate_a);
        crates.insert(TEST_CRATE_B.clone(), crate_b);
        crates.insert(TEST_CRATE_C.clone(), crate_c);

        Db::new(crates)
    }

    /// Look up a crate data.
    /// Panics if crate data is not present.
    pub fn crate_data(&self, id: &CrateId) -> &CrateData {
        // note: takes &self, doesn't need to lock a DashMap
        self.crate_data
            .get(id)
            .expect("invariant violated: no such crate")
    }

    /// Look up a parsed crate.
    /// Panics if parsed crate is not present. (Don't get ahead on the DAG!)
    pub fn get_crate(&self, id: &CrateId) -> &Crate {
        self.crates
            .get(id)
            .expect("invariant violated: no such crate")
            .get()
            .expect("invariant violated: crate has not been lowered")
    }

    /// Insert a parsed crate.
    /// Panics if the crate has already been added.
    ///
    pub fn insert_crate(&self, crate_: Crate) {
        let id = crate_.id.clone();
        let result = self
            .crates
            .get(&id)
            .expect("invariant violated: no such crate")
            .set(crate_);
        if let Err(crate_) = result {
            panic!("crate already set: {:?}", crate_.id);
        }
    }

    /*
    /// Get an item.
    pub fn get_item<I: NamespaceLookup>(&mut self, id: &Identity) -> Option<Ref<I>> {
        I::get_namespace(self.0).0.get(id)
    }

    /// Get an item mutably.
    pub fn get_item_mut<I: NamespaceLookup>(&mut self, id: &Identity) -> Option<RefMut<I>> {
        I::get_namespace(self.0).0.get_mut(id)
    }

    /// Get a prelude.
    pub fn get_prelude(&mut self, crate_: &CrateId) -> Option<Ref<Prelude, CrateId>> {
        self.0.preludes.get(crate_)
    }

    /// Add a prelude. Cannot be modified once added.
    pub fn add_prelude(&mut self, crate_: CrateId, prelude: Prelude) -> Result<(), DatabaseError> {
        match self.0.preludes.entry(crate_) {
            DEntry::Occupied(_) => Err(DatabaseError::PreludeAlreadyPresent),
            DEntry::Vacant(vac) => {
                vac.insert(prelude);
                Ok(())
            }
        }
    }

    /// Add the root scope for a crate.
    pub fn add_root_scope(
        &mut self,
        crate_: CrateId,
        scope: Scope,
    ) -> Result<Identity, DatabaseError> {
        assert!(
            &scope.metadata.name == &*ROOT_SCOPE_NAME,
            "root scope must be named `{root}`"
        );
        let root_id = Identity::root(&crate_);
        match self.0.scopes.0.entry(root_id.clone()) {
            DEntry::Occupied(_) => Err(DatabaseError::ItemAlreadyPresent),
            DEntry::Vacant(vac) => {
                vac.insert(scope);
                Ok(root_id)
            }
        }
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
    */
}

/// A parsed and resolved crate.
#[derive(Serialize, Deserialize)]
pub struct Crate {
    /// Redundancy.
    pub id: CrateId,

    /// The crate prelude.
    pub prelude: Prelude,

    /// Types in the crate.
    pub types: Namespace<TypeItem>,

    /// Symbols in the crate (functions, statics, constants)
    pub symbols: Namespace<SymbolItem>,

    /// Macros in the crate.
    pub macros: Namespace<MacroItem>,

    /// All the scopes available.
    pub scopes: Namespace<Scope>,
}

impl Crate {
    pub fn get<I: NamespaceLookup>(&self, identity: &Identity) -> Option<&I> {
        assert_eq!(self.id, identity.crate_, "cannot get outside crate!");

        I::get_namespace(self).0.get(&identity.path[..])
    }

    pub fn get_mut<I: NamespaceLookup>(&mut self, identity: &Identity) -> Option<&I> {
        assert_eq!(self.id, identity.crate_, "cannot get outside crate!");

        I::get_namespace(self).0.get(&identity.path[..])
    }
}

/// A namespace within a crate.
///
/// Invariant: if `namespace[I] == item`, `I[-1] == item.metadata().name`, UNLESS
/// `I == []`, i.e. it is a crate root.
#[derive(Serialize, Deserialize)]
pub struct Namespace<I>(pub Map<Vec<Ident>, I>);

impl<I: NamespaceLookup> Namespace<I> {
    fn new() -> Self {
        Namespace(Map::default())
    }
}

/// Generic helper.
pub trait NamespaceLookup: HasMetadata + Sized + 'static {
    fn namespace_id() -> NamespaceId;
    fn get_namespace(crate_: &Crate) -> &Namespace<Self>;
    fn get_namespace_mut(crate_: &mut Crate) -> &mut Namespace<Self>;
}
impl NamespaceLookup for TypeItem {
    fn namespace_id() -> NamespaceId {
        NamespaceId::Type
    }
    fn get_namespace(crate_: &Crate) -> &Namespace<Self> {
        &crate_.types
    }

    fn get_namespace_mut(crate_: &mut Crate) -> &mut Namespace<Self> {
        &mut crate_.types
    }
}
impl NamespaceLookup for SymbolItem {
    fn namespace_id() -> NamespaceId {
        NamespaceId::Symbol
    }
    fn get_namespace(crate_: &Crate) -> &Namespace<Self> {
        &crate_.symbols
    }
    fn get_namespace_mut(crate_: &mut Crate) -> &mut Namespace<Self> {
        &mut crate_.symbols
    }
}
impl NamespaceLookup for MacroItem {
    fn namespace_id() -> NamespaceId {
        NamespaceId::Type
    }
    fn get_namespace(crate_: &Crate) -> &Namespace<Self> {
        &crate_.macros
    }
    fn get_namespace_mut(crate_: &mut Crate) -> &mut Namespace<Self> {
        &mut crate_.macros
    }
}
impl NamespaceLookup for Scope {
    fn namespace_id() -> NamespaceId {
        NamespaceId::Scope
    }
    fn get_namespace(crate_: &Crate) -> &Namespace<Self> {
        &crate_.scopes
    }
    fn get_namespace_mut(crate_: &mut Crate) -> &mut Namespace<Self> {
        &mut crate_.scopes
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
        PreludeAlreadyPresent {
            display("crate already has a prelude")
        }
    }
}

#[cfg(test)]
mod tests {}
