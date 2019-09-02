//! The Namespace data structure.

use super::ResolveError;
use crate::Set;
use dashmap::{DashMap, DashMapRefAny};
use transgress_api::idents::Ident;
use transgress_api::paths::{AbsolutePath, AbsoluteCrate};

/// A namespace, for holding some particular type of item during resolution.
/// Allows operating on many different items in parallel.
pub struct Namespace<I: Namespaced> {
    items: DashMap<AbsolutePath, I>,
    module_map: DashMap<AbsolutePath, Set<Ident>>,
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
            module_map: DashMap::default(),
        }
    }

    /// Insert an item into the namespace.
    pub fn insert(&self, path: AbsolutePath, item: I) -> Result<(), ResolveError> {
        let mut failed = true;

        {
            self.items.get_or_insert_with(&path, || {
                failed = false;
                item
            });
        }
        let result = if failed {
            Err(ResolveError::AlreadyDefined(I::namespace(), path.clone()))
        } else {
            Ok(())
        };

        self.update_module_map(path);

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

    /// Add a path to it's parents list of sub-paths.
    /// Noop for root crate entry.
    fn update_module_map(&self, mut path: AbsolutePath) {
        if let Some(last) = path.path.pop() {
            {
                // get a mutable reference
                if let DashMapRefAny::Unique(mut mut_) =
                self.module_map.get_or_insert_with(&path, Set::default)
                {
                    // if we get a mutable ref, use it
                    mut_.insert(last);
                    return;
                }
            }

            {
                // otherwise, get one explicitly
                self.module_map.index_mut(&path).insert(last);
            };
        }
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

    /// Return if the namespace contains a path.
    pub fn contains(&self, path: &AbsolutePath) -> bool {
        self.items.contains_key(path)
    }

    /// Iterate through all the known paths in a module.
    pub fn iter_module<'a>(
        &'a self,
        module: &'a AbsolutePath,
    ) -> impl Iterator<Item = AbsolutePath> + 'a {
        self.module_map
            .get(module)
            .into_iter()
            .flat_map(move |entries| {
                entries
                    .clone()
                    .into_iter()
                    .map(move |entry| module.clone().join(entry))
            })
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


#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolver::ModuleImports;
    use transgress_api::paths::AbsoluteCrate;

    fn fake_a() -> AbsolutePath {
        AbsolutePath {
            crate_: AbsoluteCrate {
                name: "core".into(),
                version: "0.0.0".into()
            },
            path: vec!["a".into()]
        }
    }
    fn fake_b() -> AbsolutePath {
        AbsolutePath {
            crate_: AbsoluteCrate {
                name: "core".into(),
                version: "0.0.0".into()
            },
            path: vec!["b".into()]
        }
    }

    #[test]
    fn insert_deadlock() {
        spoor::init();

        let namespace = Namespace::<ModuleImports>::new();

        // this used to have a deadlock, doesn't anymore
        namespace.insert(fake_a(), ModuleImports::new()).unwrap();
        namespace.insert(fake_b(), ModuleImports::new()).unwrap();
    }
}
