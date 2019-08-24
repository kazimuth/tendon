//! The Namespace data structure.

use transgress_api::attributes::{HasMetadata, Metadata};
use transgress_api::paths::AbsolutePath;
use crate::Map;
use parking_lot::Mutex;
use super::ResolveError;
use tracing::error;

/// A namespace, for holding some particular type of item during resolution.
/// Consists of a hashmap of absolute paths to RwLocks of entries.
pub struct Namespace<I: Namespaced>(Map<AbsolutePath, Mutex<I>>);
impl<I: Namespaced> Namespace<I> {
    /// Create a namespace.
    pub fn new() -> Self {
        Namespace(Map::default())
    }

    /// Insert an item into the namespace.
    pub fn insert(&mut self, path: AbsolutePath, item: I) {
        self.insert_impl(path, Mutex::new(item));
    }

    /// Modify the item present at a path.
    /// Note, this takes &self: you can modify multiple items at a time.
    pub fn modify<F: FnOnce(&mut I) -> Result<(), ResolveError>>(&self, path: &AbsolutePath, f: F) -> Result<(), ResolveError> {
        if let Some(item) = self.0.get(path) {
            let mut lock = item.lock();
            f(&mut *lock)
        } else {
            Err(ResolveError::PathNotFound(I::namespace(), path.clone()))
        }
    }

    /// Return if the namespace contains a path.
    pub fn contains(&self, path: &AbsolutePath) -> bool {
        self.0.contains_key(path)
    }

    /// Merge values from another namespace.
    pub fn merge(&mut self, other: Namespace<I>) {
        for (path, item) in other.0 {
            self.insert_impl(path, item);
        }
    }

    /// insertion helper.
    fn insert_impl(&mut self, path: AbsolutePath, item: Mutex<I>) -> Result<(), ResolveError> {
        let entry = self.0.entry(path);
        match entry {
            hashbrown::hash_map::Entry::Occupied(occ) => {
                occ.get().lock().merge(item.into_inner())
            }
            hashbrown::hash_map::Entry::Vacant(vac) => {
                vac.insert(item);
                Ok(())
            }
        }
    }
}

pub trait Namespaced {
    fn namespace() -> &'static str;
    fn merge(&mut self, other: Self) -> Result<(), ResolveError>;
}

impl Namespaced for transgress_api::items::TypeItem {
    fn namespace() -> &'static str {
        "type"
    }
    fn merge(&mut self, other: Self) -> Result<(), ResolveError> {
        Err(ResolveError::AlreadyDefined())
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
impl Namespaced for super::Scope {
    fn namespace() -> &'static str {
        "scope"
    }
}
