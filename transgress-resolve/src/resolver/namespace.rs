//! The Namespace data structure.

use super::ResolveError;
use crate::Map;
use cargo_metadata::Resolve;
use parking_lot::Mutex;
use std::error::Error;
use std::path::PathBuf;
use transgress_api::attributes::{HasMetadata, Span};
use transgress_api::paths::AbsolutePath;

/// A namespace, for holding some particular type of item during resolution.
/// Allows operating on many different items in parallel.
/// Every path in the namespace can be marked invalid, meaning that something related to that
/// item has caused an error (i.e. parse failure, unimplemented macro expansion, something else).
/// Items depending on invalid items should be marked invalid as well.
/// (Invalid items are represented internally as Nones.)
pub struct Namespace<I: Namespaced>(Map<AbsolutePath, Mutex<Option<I>>>);
impl<I: Namespaced> Namespace<I> {
    /// Create a namespace.
    pub fn new() -> Self {
        Namespace(Map::default())
    }

    /// Insert an item into the namespace.
    pub fn insert(&mut self, path: AbsolutePath, item: I) -> Result<(), ResolveError> {
        self.insert_impl(path, Mutex::new(Some(item)))
    }

    /// Mark an item as invalid.
    pub fn mark_invalid(&mut self, path: AbsolutePath) -> Result<(), ResolveError> {
        self.insert_impl(path, Mutex::new(None))
    }

    /// Modify the item present at a path.
    /// If the modification returns an error, this will invalidate the item.
    pub fn modify<F: FnOnce(&mut I) -> Result<(), ResolveError>>(
        &self,
        path: &AbsolutePath,
        f: F,
    ) -> Result<(), ResolveError> {
        if let Some(item) = self.0.get(path) {
            let mut lock = item.lock();
            let err = if let Some(item) = &mut *lock {
                if let Err(err) = f(item) {
                    err
                } else {
                    return Ok(());
                }
            } else {
                return Err(ResolveError::CachedError(path.clone()));
            };
            *lock = None;

            Err(err)
        } else {
            Err(ResolveError::PathNotFound(I::namespace(), path.clone()))
        }
    }

    /// Return if the namespace contains a path.
    pub fn contains(&self, path: &AbsolutePath) -> bool {
        self.0.contains_key(path)
    }

    /// Insertion helper.
    fn insert_impl(
        &mut self,
        path: AbsolutePath,
        item: Mutex<Option<I>>,
    ) -> Result<(), ResolveError> {
        let entry = self.0.entry(path);
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
