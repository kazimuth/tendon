//! The Namespace data structure.

use crate::resolver::ResolveError;
use crate::walker::WalkError;
use dashmap::{DashMap, DashMapRefAny};
use tendon_api::idents::Ident;
use tendon_api::paths::{AbsoluteCrate, AbsolutePath};

/// A namespace, for holding some particular type of item during resolution.
/// Allows operating on many different items in parallel.
pub struct Namespace<I: Namespaced> {
    items: DashMap<AbsolutePath, I>,
    // note: could change this to contain a dashmap as well...
    crate_map: DashMap<AbsoluteCrate, DashMap<Vec<Ident>, ()>>,
}

// Modifying multiple items in a namespace at the same time can deadlock.
// This will detect this condition during te
#[cfg(test)]
#[macro_use]
mod check_modify {
    use std::cell::Cell;
    thread_local! {
        static CALLING_MODIFY: Cell<bool> = Cell::new(false);
    }

    pub struct CallingModify(());

    pub fn check_modify() -> CallingModify {
        CALLING_MODIFY.with(|cm| {
            assert!(
                !cm.get(),
                "recursively modifying database can deadlock, please don't"
            );
            cm.set(true);

            CallingModify(())
        })
    }

    impl Drop for CallingModify {
        fn drop(&mut self) {
            CALLING_MODIFY.with(|cm| {
                cm.set(false);
            });
        }
    }

    macro_rules! check_modify {
        () => {
            let _check = check_modify::check_modify();
        };
    }
}
#[cfg(not(test))]
macro_rules! check_modify {
    () => {};
}

impl<I: Namespaced> Namespace<I> {
    /// Create a namespace.
    pub fn new() -> Self {
        Namespace {
            items: DashMap::default(),
            crate_map: DashMap::default(),
        }
    }

    /// Insert an item into the namespace.
    pub fn insert(&self, path: AbsolutePath, item: I) -> Result<(), WalkError> {
        let mut failed = true;

        {
            self.items.get_or_insert_with(&path, || {
                failed = false;
                item
            });
        }
        let result = if failed {
            Err(WalkError::AlreadyDefined(I::namespace(), path.clone()))
        } else {
            Ok(())
        };

        self.update_crate_map(path);

        result
    }

    pub fn insert_or_update<F>(
        &self,
        path: AbsolutePath,
        item: I,
        mut f: F,
    ) -> Result<(), ResolveError>
    where
        F: FnMut(&mut I, I) -> Result<(), ResolveError>,
        I: Clone,
    {
        if let Some(mut current) = self.items.get_mut(&path) {
            check_modify!();
            f(&mut *current, item)
        } else {
            let mut failed = true;
            // TODO: can we avoid this clone?
            let cloned = item.clone();

            self.items.get_or_insert_with(&path, || {
                failed = false;
                cloned
            });

            if failed {
                // try again
                self.insert_or_update(path, item, f)
            } else {
                Ok(())
            }
        }
    }

    /// Add a path to the list of paths for each crate.
    fn update_crate_map(&self, path: AbsolutePath) {
        let AbsolutePath { crate_, path } = path;
        let submap = self.crate_map.get_or_insert_with(&crate_, DashMap::default);
        submap.insert(path, ());
    }

    /// Modify the item present at a path.
    /// If the modification fails, you might want to remove the item.
    /// Note: calling this recursively can deadlock!!
    pub fn modify<R, F: FnOnce(&mut I) -> Result<R, ResolveError>>(
        &self,
        path: &AbsolutePath,
        f: F,
    ) -> Result<R, ResolveError> {
        if let Some(mut item) = self.items.get_mut(&path) {
            check_modify!();
            f(&mut *item)
        } else {
            Err(ResolveError::PathNotFound(I::namespace(), path.clone()))
        }
    }

    /// Inspect the item present at a path.
    /// If the modification fails, you might want to remove the item.
    /// Note: calling this and `modify` at the same time can deadlock!!
    pub fn inspect<R, F: FnOnce(&I) -> Result<R, ResolveError>>(
        &self,
        path: &AbsolutePath,
        f: F,
    ) -> Result<R, ResolveError> {
        if let Some(item) = self.items.get(&path) {
            check_modify!();
            f(&*item)
        } else {
            Err(ResolveError::PathNotFound(I::namespace(), path.clone()))
        }
    }

    /// Temporarily remove an item from the database to modify it.
    /// This will allow you to inspect other items in the database while modifying the item.
    /// However, your item will be mysteriously gone from the database.
    /// This is only good to use when you *know* nothing else wants to look for your item.
    pub fn take_modify<R, F: FnOnce(&mut I) -> Result<R, ResolveError>>(
        &self,
        path: &AbsolutePath,
        f: F,
    ) -> Result<R, ResolveError> {
        if let Some((path, mut item)) = self.items.remove(&path) {
            let result = f(&mut item);
            self.items.insert(path, item);
            result
        } else {
            Err(ResolveError::PathNotFound(I::namespace(), path.clone()))
        }
    }

    /// Return if the namespace contains a path.
    pub fn contains(&self, path: &AbsolutePath) -> bool {
        self.items.contains_key(path)
    }

    /// Iterate through all the known paths in a crate.
    pub fn iter_crate<'a>(
        &'a self,
        crate_: &'a AbsoluteCrate,
    ) -> impl Iterator<Item = AbsolutePath> + 'a {
        self.crate_map
            .get(crate_)
            .into_iter()
            .flat_map(move |entries| {
                entries
                    .iter()
                    .map(move |entry| AbsolutePath::new(crate_.clone(), &entry.key()[..]))
                    .collect::<Vec<_>>()
                    .into_iter()
            })
    }

    /// How many items are in this namespace?
    pub fn len(&self) -> usize {
        self.items.len()
    }

    // TODO remove
}

pub trait Namespaced {
    fn namespace() -> &'static str;
}

impl Namespaced for tendon_api::items::TypeItem {
    fn namespace() -> &'static str {
        "type"
    }
}
impl Namespaced for tendon_api::items::SymbolItem {
    fn namespace() -> &'static str {
        "symbol"
    }
}
impl Namespaced for tendon_api::items::MacroItem {
    fn namespace() -> &'static str {
        "macro"
    }
}
impl Namespaced for tendon_api::items::ModuleItem {
    fn namespace() -> &'static str {
        "module"
    }
}
impl Namespaced for crate::walker::ModuleScope {
    fn namespace() -> &'static str {
        "[implementation detail] scope"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::walker::ModuleScope;
    use tendon_api::paths::AbsoluteCrate;

    fn fake_a() -> AbsolutePath {
        AbsolutePath::new(AbsoluteCrate::new("core", "0.0.0"), &["a"])
    }
    fn fake_b() -> AbsolutePath {
        AbsolutePath::new(AbsoluteCrate::new("core", "0.0.0"), &["b"])
    }

    #[test]
    fn insert_deadlock() {
        spoor::init();

        let namespace = Namespace::<ModuleScope>::new();

        // this used to have a deadlock, doesn't anymore
        namespace.insert(fake_a(), ModuleScope::new()).unwrap();
        namespace.insert(fake_b(), ModuleScope::new()).unwrap();
    }
}
