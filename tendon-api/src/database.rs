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

use crate::attributes::{HasMetadata, Metadata, Visibility};
use crate::crates::CrateData;
use crate::identities::{CrateId, Identity, TEST_CRATE_A, TEST_CRATE_B, TEST_CRATE_C};
use crate::items::{MacroItem, SymbolItem, TypeItem};
use crate::paths::Ident;
use crate::scopes::{Binding, NamespaceId, Priority, Scope};
use crate::Map;
use hashbrown::hash_map::Entry as HEntry;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use tracing::error;

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
}

/// A parsed and resolved crate.
#[derive(Serialize, Deserialize)]
pub struct Crate {
    /// Redundancy.
    pub id: CrateId,

    /// A prelude. They mostly act like a normal scope, but they apply to a whole crate. Names
    /// are not exported.
    ///
    /// Not to be confused with the "language prelude", i.e. std::prelude::v1 -- that is a set of names
    /// that's *added* to each crate's prelude. However, a crate prelude can include other items as well;
    /// notably, extern crates, and macros imported with macro_use!.
    ///
    /// The `#[no_implicit_prelude]` disables the entire crate prelude for some module, including extern crates!
    /// External crates must be accessed like `::krate` to work in a no_implicit_prelude module.
    ///
    /// By default, crates have an empty prelude. See `tendon_resolve::walker::helpers::build_prelude`
    /// for code that adds all the relevant names.
    pub prelude: Scope,

    /// Extern crate bindings. Used to look up paths starting with `::`.
    /// May have extra bindings added in source code; `extern crate a as b` adds *both* `a` and `b`
    /// to this dict, binding to the `a` crate.
    pub extern_crate_bindings: Map<Ident, CrateId>,

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
    /// Create an empty crate.
    pub fn new(id: CrateId) -> Crate {
        Crate {
            id,
            extern_crate_bindings: Map::default(),
            prelude: Scope::new(Metadata::fake("{prelude}"), false),
            types: Namespace::new(),
            symbols: Namespace::new(),
            macros: Namespace::new(),
            scopes: Namespace::new(),
        }
    }

    /// Look up an Identity in a namespace (NOT a binding.)
    /// The Identity must be in this crate.
    pub fn get<I: NamespaceLookup>(&self, identity: &Identity) -> Option<&I> {
        assert_eq!(self.id, identity.crate_, "cannot get outside crate!");

        I::get_namespace(self).0.get(&identity.path[..])
    }

    /// Look up an Identity in a namespace (NOT a binding.)
    /// The Identity must be in this crate.
    pub fn get_mut<I: NamespaceLookup>(&mut self, identity: &Identity) -> Option<&mut I> {
        assert_eq!(self.id, identity.crate_, "cannot get outside crate!");

        I::get_namespace_mut(self).0.get_mut(&identity.path[..])
    }

    /// Look up a binding in a scope. Checks prelude as well. Does not check visibilities.
    pub fn get_binding<I: NamespaceLookup>(
        &self,
        containing_scope: &Identity,
        name: &Ident,
    ) -> Option<&Binding> {
        self.get_binding_by(containing_scope, I::namespace_id(), name)
    }

    /// Look up a binding in a scope. Checks prelude as well. Does not check visibilities.
    #[inline(never)]
    pub fn get_binding_by(
        &self,
        containing_scope: &Identity,
        namespace_id: NamespaceId,
        name: &Ident,
    ) -> Option<&Binding> {
        let scope = if let Some(scope) = self.get::<Scope>(containing_scope) {
            scope
        } else {
            error!("no such scope: {:?}", containing_scope);
            return None;
        };
        if let Some(name) = scope.get_by(namespace_id, name) {
            return Some(name);
        }
        if let Some(prelude) = self.prelude.get_by(namespace_id, name) {
            return Some(prelude);
        }
        None
    }

    /// Add an item to a crate. The item is added at `{containing_scope}::{item.metadata().name}`.
    /// The containing_scope must be in this crate.
    /// Also adds a Binding to the containing scope.
    pub fn add<I: NamespaceLookup>(
        &mut self,
        containing_scope: &Identity,
        item: I,
    ) -> Result<Identity, DatabaseError> {
        assert_eq!(
            containing_scope.crate_, self.id,
            "can't add item to a different crate!!"
        );

        let name = item.metadata().name.clone();
        let visibility = item.metadata().visibility.clone();

        let identity = containing_scope.clone_join(name.clone());
        let Identity { path, crate_ } = identity.clone();

        match I::get_namespace_mut(self).0.entry(path) {
            HEntry::Vacant(vacant) => {
                vacant.insert(item);
            }
            HEntry::Occupied(occupied) => {
                error!(
                    "{:?} ({:?}) already defined!",
                    Identity::new(&crate_, occupied.key()),
                    I::namespace_id()
                );
                return Err(DatabaseError::ItemAlreadyPresent);
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

    /// Add the root scope.
    pub fn add_root_scope(&mut self, metadata: Metadata) -> Result<Identity, DatabaseError> {
        assert!(&metadata.name[..] == "{root}");
        let item = Scope::new(metadata, true);

        match self.scopes.0.entry(vec![]) {
            HEntry::Vacant(vacant) => {
                vacant.insert(item);
                Ok(Identity::root(&self.id))
            }
            HEntry::Occupied(_) => {
                error!("{:?} root already defined!", &self.id);
                Err(DatabaseError::ItemAlreadyPresent)
            }
        }
    }

    /// Add a binding to a scope. Doesn't have to target something in this crate.
    pub fn add_binding<I: NamespaceLookup>(
        &mut self,
        containing_scope: &Identity,
        name: Ident,
        target: Identity,
        visibility: Visibility,
        priority: Priority,
    ) -> Result<(), DatabaseError> {
        self.add_binding_by(
            containing_scope,
            I::namespace_id(),
            name,
            target,
            visibility,
            priority,
        )
    }

    #[inline(never)]
    pub fn add_binding_by(
        &mut self,
        containing_scope: &Identity,
        namespace_id: NamespaceId,
        name: Ident,
        target: Identity,
        visibility: Visibility,
        priority: Priority,
    ) -> Result<(), DatabaseError> {
        println!(
            "add_binding_by {:?} {:?} {:?} {:?}",
            containing_scope, namespace_id, name, target
        );
        assert!(
            &containing_scope.crate_ == &self.id,
            "can't add a binding to another crate!"
        );

        let scope = self
            .get_mut::<Scope>(containing_scope)
            .ok_or(DatabaseError::NoSuchScope)?;

        // TODO allow replacing bindings somehow?
        scope
            .insert_by(namespace_id, name, target, visibility, priority)
            .map_err(|_| DatabaseError::BindingAlreadyPresent)
    }

    /// Add a binding to the prelude.
    pub fn add_prelude_binding_by(
        &mut self,
        namespace_id: NamespaceId,
        name: Ident,
        target: Identity,
    ) -> Result<(), DatabaseError> {
        let visibility = Visibility::InScope(Identity::root(&self.id));
        self.prelude
            .insert_by(namespace_id, name, target, visibility, Priority::Explicit)
            .map_err(|_| DatabaseError::BindingAlreadyPresent)
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
        NamespaceId::Macro
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
            display("item is already present")
        }
        BindingAlreadyPresent {
            display("binding is already present")
        }
        NoSuchScope {
            display("no such scope")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attributes::TypeMetadata;
    use crate::items::{EnumItem, GenericParams};

    #[test]
    fn crate_building() {
        let test_crate_a = (*TEST_CRATE_A).clone();

        let mut crate_ = Crate::new(test_crate_a.clone());
        let root = crate_.add_root_scope(Metadata::fake("{root}")).unwrap();

        assert!(crate_.get::<Scope>(&root).is_some());

        let mod_a = crate_
            .add(&root, Scope::new(Metadata::fake("a"), true))
            .unwrap();

        assert!(crate_.get::<Scope>(&mod_a).is_some());
        assert!(
            &crate_
                .get_binding::<Scope>(&root, &"a".into())
                .unwrap()
                .identity
                == &mod_a
        );

        let mod_a_b = crate_
            .add(&mod_a, Scope::new(Metadata::fake("a"), true))
            .unwrap();

        let type_a_b_c = crate_
            .add(
                &mod_a_b,
                TypeItem::Enum(EnumItem {
                    metadata: Metadata::fake("C"),
                    type_metadata: TypeMetadata::default(),
                    generic_params: GenericParams::default(),
                    variants: vec![],
                }),
            )
            .unwrap();

        crate_
            .add_binding::<TypeItem>(
                &root,
                "CRenamed".into(),
                type_a_b_c.clone(),
                Visibility::InScope(root.clone()),
                Priority::Explicit,
            )
            .unwrap();

        let in_root = crate_
            .get_binding::<TypeItem>(&root, &"CRenamed".into())
            .unwrap();
        let in_a_b = crate_
            .get_binding::<TypeItem>(&mod_a_b, &"C".into())
            .unwrap();

        assert_eq!(in_root.identity, in_a_b.identity);
        assert_eq!(in_root.visibility, Visibility::InScope(root.clone()));
        assert_eq!(in_a_b.visibility, Visibility::Pub);
    }
}
