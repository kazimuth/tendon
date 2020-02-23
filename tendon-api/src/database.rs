//! The Database data structure, which holds everything important and is simple to serialize to disk.

use crate::attributes::Visibility;
use crate::paths::{AbsoluteCrate, AbsolutePath, RelativePath};
use crate::Map;
use hashbrown::hash_map::Entry;
use serde::{Deserialize, Serialize};

/// A namespace within a crate, for holding some particular type of item during resolution.
#[derive(Serialize, Deserialize)]
pub struct CrateNamespace<I> {
    /// The crate for this namespace.
    /// (stored redundantly for convenience.)
    crate_: AbsoluteCrate,

    /// The type of this namespace (e.g. "type", "symbol", "macro").
    /// Used for printing debug messages.
    namespace_type: String,

    /// Original, stored by the scope where they're defined.
    items: Map<RelativePath, I>,

    /// Reexports.
    /// Note that every item has a reexport added corresponding to itself.
    /// Note also that these are collapsed: if you have `a reexports b reexports c`, this should map `a`
    /// to `c`, skipping `b`. This property is easy enough to ensure by construction.
    reexports: Map<RelativePath, Reexport>,
}

impl<I> CrateNamespace<I> {
    /// Create a namespace within a crate.
    pub fn new(crate_: AbsoluteCrate, namespace_type: &str) -> Self {
        CrateNamespace {
            crate_,
            namespace_type: namespace_type.to_string(),
            items: Map::default(),
            reexports: Map::default(),
        }
    }

    /// Insert an item, and add a reexport for that item in the relevant module.
    ///
    /// - `visibility`: the item's intrinsic visibility.
    /// - `module_external_visibility`: whether the module containing the item is externally
    ///   visible, i.e. it and all of its parents are `pub`.
    pub fn insert(
        &mut self,
        path: RelativePath,
        item: I,
        visibility: Visibility,
        module_external_visibility: Visibility,
    ) -> Result<(), DatabaseError> {
        match self.items.entry(path.clone()) {
            Entry::Occupied(_) => return Err(DatabaseError::ItemAlreadyPresent),
            Entry::Vacant(v) => v.insert(item),
        };

        let external_visibility = match (visibility, module_external_visibility) {
            (Visibility::Pub, Visibility::Pub) => Visibility::Pub,
            _ => Visibility::NonPub,
        };

        let reexport = Reexport {
            absolute_target: AbsolutePath::new(self.crate_.clone(), &path.0),
            inner_visibility: visibility,
            external_visibility,
        };

        match self.reexports.entry(path) {
            // TODO there may be some other way to resolve ambiguities in this case? something w/ globs?
            Entry::Occupied(_) => return Err(DatabaseError::ReexportAlreadyPresent),
            Entry::Vacant(v) => v.insert(reexport),
        };

        Ok(())
    }

    /// Rust allows public items to be defined in private modules. We address these items by these
    /// private modules, but that means that when it's time to access them in codegen, we might not
    /// be able to find them.
    /// So instead we use this slightly silly algorithm to pick some other accessible path.
    /// Currently we select the shortest, alphabetically first path in the source crate.
    /// Note that public items without public reexports will be ignored here, that's the correct behavior.
    pub fn get_canonical_paths(&self) -> Map<AbsolutePath, CanonicalPath> {
        let mut canonical_paths: Map<AbsolutePath, CanonicalPath> = Map::default();

        for (path, reexport) in &self.reexports {
            if reexport.external_visibility != Visibility::Pub {
                continue;
            }
            if reexport.absolute_target.crate_ != self.crate_ {
                continue;
            }

            if let Some(canonical) = canonical_paths.get_mut(&reexport.absolute_target) {
                if canonical.0.path.0.len() < reexport.absolute_target.path.0.len()
                    && canonical.0.path < reexport.absolute_target.path
                {
                    canonical.0 = reexport.absolute_target.clone();
                }
            } else {
                canonical_paths.insert(
                    reexport.absolute_target.clone(),
                    CanonicalPath(AbsolutePath::new(self.crate_.clone(), &path.0)),
                );
            }
        }

        canonical_paths
    }
}
// Helper newtype.
pub struct CanonicalPath(pub AbsolutePath);

/// A re-exported item.
#[derive(Serialize, Deserialize)]
pub struct Reexport {
    /// The original, true path of the reexported item.
    pub absolute_target: AbsolutePath,

    /// The visibility of the reexport (NOT the item), in the scope of its module.
    pub inner_visibility: Visibility,

    /// Whether the reexport is visible from outside the crate.
    pub external_visibility: Visibility,
}

quick_error::quick_error! {
    #[derive(Debug)]
    pub enum DatabaseError {
        ItemAlreadyPresent {
            display("item has already been added?")
        }
        ReexportAlreadyPresent {
            display("item is already reexported?")
        }
    }
}

/*
/// A database of everything found within a crate.
pub struct CrateDb {
    crate_: AbsoluteCrate,

    pub types: Namespace<TypeItem>,
    pub symbols: Namespace<SymbolItem>,
    pub macros: Namespace<MacroItem>,
    /// `mod` items, mostly just store metadata.
    pub modules: Namespace<ModuleItem>,
}

impl Db {
    /// Create a new database.
    pub fn new() -> Db {
        Db {
            types: namespace::Namespace::new(),
            symbols: namespace::Namespace::new(),
            macros: namespace::Namespace::new(),
            modules: namespace::Namespace::new(),
        }
    }
}
*/
