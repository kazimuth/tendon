//! The internal database of paths.
use crate::{
    items::{MacroItem, SymbolItem, TypeItem},
    paths::{AbsoluteCrate, AbsolutePath},
};
// TODO: faster hashmap
use std::collections::{HashMap, HashSet};
use tracing::warn;

// ops:
// - purge: remove all entries w/ some property
//   - can be implemented as layer above, "only iterate things w/ these properties"
// - insert:
// - modify: yield mut ref to entry

// do we want floating MethodItems? they... sorta have canonical paths? kinda same problem as traits
// -> attach extra scope information to some things

// macro name resolution is affected by order, right?
// -> a macro's meaning can't change by adding new rules, because if something would have matched before,
//    it'll still match after
// is it possible to view a macro's rules as having been declared in a different order?
// https://rust-lang.github.io/rustc-guide/name-resolution.html
// https://github.com/rust-lang/rust/blob/master/src/librustc_resolve/lib.rs

// TODO: refactor to reuse code between namespaces?
// TODO: refactor to use fast thread-safe data structures (e.g. ccl)?

/// The main entry-point to this crate: a database of known paths.
pub struct Db {
    macros: HashMap<AbsolutePath, MacroItem>,
    symbols: HashMap<AbsolutePath, SymbolItem>,
    types: HashMap<AbsolutePath, TypeItem>,
    crate_contents: HashMap<AbsoluteCrate, HashSet<AbsolutePath>>,
}

impl Db {
    pub fn new() -> Db {
        Db {
            macros: HashMap::new(),
            symbols: HashMap::new(),
            types: HashMap::new(),
            crate_contents: HashMap::new(),
        }
    }

    fn link_crate(&mut self, path: &AbsolutePath) {
        if let Some(contents) = self.crate_contents.get_mut(&path.crate_) {
            contents.insert(path.clone());
        } else {
            let mut contents = HashSet::new();
            contents.insert(path.clone());
            self.crate_contents.insert(path.crate_.clone(), contents);
        }
    }

    /// Insert a macro.
    /// Note that, if the macro is declarative, it's permitted to call this multiple times. The rules
    /// will be aggregated into a single macro_rules declaration.
    /// (this isn't strictly speaking correct but until i run into something that needs proper macro scoping i cba to implement it.)
    pub fn insert_macro(&mut self, name: &AbsolutePath, item: MacroItem) {
        self.link_crate(&name);

        if let Some(_entry) = self.symbols.get_mut(name) {
            unimplemented!("append rules")
        } else {
            self.macros.insert(name.clone(), item);
        }
    }

    /// Insert a symbol.
    pub fn insert_symbol(&mut self, name: &AbsolutePath, item: SymbolItem) {
        self.link_crate(name);

        if let Some(_) = self.symbols.insert(name.clone(), item) {
            warn!("displacing previous symbol at path {:?}", name);
        }
    }

    /// Insert a type or trait.
    pub fn insert_type(&mut self, name: &AbsolutePath, item: TypeItem) {
        self.link_crate(name);

        if let Some(_) = self.types.insert(name.clone(), item) {
            warn!("displacing previous symbol at path {:?}", name);
        }
    }
}
