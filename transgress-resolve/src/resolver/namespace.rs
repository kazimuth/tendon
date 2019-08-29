//! The Namespace data structure.

use super::ResolveError;
use crate::Map;
use parking_lot::Mutex;
use tracing::warn;
use transgress_api::idents::Ident;
use transgress_api::paths::AbsolutePath;

/// A namespace, for holding some particular type of item during resolution.
/// Allows operating on many different items in parallel.
/// Every path in the namespace can be marked invalid, meaning that something related to that
/// item has caused an error (i.e. parse failure, unimplemented macro expansion, something else).
/// Items depending on invalid items should be marked invalid as well.
/// (Invalid items are represented internally as Nones.)
pub struct Namespace<I: Namespaced> {
    items: Map<AbsolutePath, Mutex<I>>,
    module_map: Map<AbsolutePath, Vec<Ident>>,
}

impl<I: Namespaced> Namespace<I> {
    /// Create a namespace.
    pub fn new() -> Self {
        Namespace {
            items: Map::default(),
            module_map: Map::default(),
        }
    }

    /// Insert an item into the namespace.
    pub fn insert(&mut self, path: AbsolutePath, item: I) -> Result<(), ResolveError> {
        self.insert_impl(path, Mutex::new(item))
    }

    /// Modify the item present at a path.
    /// If the modification fails, you might want to remove the item.
    pub fn modify<F: FnOnce(&mut I) -> Result<(), ResolveError>>(
        &self,
        path: &AbsolutePath,
        f: F,
    ) -> Result<(), ResolveError> {
        if let Some(item) = self.items.get(path) {
            let mut lock = item.lock();
            if let Err(err) = f(&mut *lock) {
                Err(err)
            } else {
                Ok(())
            }
        } else {
            Err(ResolveError::PathNotFound(I::namespace(), path.clone()))
        }
    }

    /// Return if the namespace contains a path.
    pub fn contains(&self, path: &AbsolutePath) -> bool {
        self.items.contains_key(path)
    }

    /// Merge all items from another namespace.
    pub fn merge_from(&mut self, other: Namespace<I>) {
        for (path, item) in other.items {
            if let Err(err) = self.insert_impl(path, item) {
                warn!("error during Db merge: {}", err);
            }
        }
    }

    /// Insertion helper.
    fn insert_impl(&mut self, path: AbsolutePath, item: Mutex<I>) -> Result<(), ResolveError> {
        let mut parent = path.clone();

        // don't modify modulemap for root crate entry
        if let Some(last) = parent.path.pop() {
            self.module_map
                .entry(parent)
                .or_insert_with(Vec::new)
                .push(last)
        }

        let entry = self.items.entry(path);
        match entry {
            hashbrown::hash_map::Entry::Occupied(occ) => Err(ResolveError::AlreadyDefined(
                I::namespace(),
                occ.key().clone(),
            )),
            hashbrown::hash_map::Entry::Vacant(vac) => {
                vac.insert(item);
                Ok(())
            }
        }
    }

    // TODO remove
}

pub trait Namespaced {
    fn namespace() -> &'static str;
}

impl Namespaced for transgress_api::items::TypeItem {
    fn namespace() -> &'static str {
        "type"
    }
}
impl Namespaced for transgress_api::items::SymbolItem {
    fn namespace() -> &'static str {
        "symbol"
    }
}
impl Namespaced for transgress_api::items::MacroItem {
    fn namespace() -> &'static str {
        "macro"
    }
}
impl Namespaced for transgress_api::items::ModuleItem {
    fn namespace() -> &'static str {
        "module"
    }
}
impl Namespaced for super::ModuleImports {
    fn namespace() -> &'static str {
        "scope"
    }
}
