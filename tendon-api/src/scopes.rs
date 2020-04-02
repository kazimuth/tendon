//! Scopes and bindings.

use crate::attributes::{Metadata, Visibility};
use crate::database::NamespaceLookup;
use crate::identities::{Identity, CrateId};
use crate::paths::Ident;
use crate::Map;
use serde::{Deserialize, Serialize};

/// A scope, containing bindings for all 4 namespaces.
///
/// Scopes may be named (modules, enums, traits) or anonymous (impl blocks, function bodies.)
/// For non-module scopes, most metadata is only stored on the item, although the span and name is stored
/// here too.
///
/// Also, we're cheeky and put lifetiems in the type namespace, but their names have ticks so
/// its basically distinct.
#[derive(Debug, Serialize, Deserialize)]
pub struct Scope {
    pub metadata: Metadata,
    pub is_module: bool,
    bindings: [Map<Ident, Binding>; 4],
}
impl Scope {
    pub fn new(metadata: Metadata, is_module: bool) -> Scope {
        Scope {
            metadata,
            is_module,
            bindings: Default::default(),
        }
    }

    /// Get the bindings for a namespace.
    pub fn get_bindings_mut<I: NamespaceLookup>(&mut self) -> &mut Map<Ident, Binding> {
        &mut self.bindings[I::namespace_id() as usize]
    }

    /// Get the bindings for a namespace.
    pub fn get_bindings<I: NamespaceLookup>(&self) -> &Map<Ident, Binding> {
        &self.bindings[I::namespace_id() as usize]
    }
}

/// A name binding. (Nothing to do with the idea of "language bindings".)
#[derive(Serialize, Deserialize, Debug)]
pub struct Binding {
    /// The final target this binding points to. If this points to another binding
    /// (e.g. a reexport), that chain is followed.
    pub identity: Identity,

    /// The visibility of the binding (NOT the item).
    pub visibility: Visibility,

    /// If the binding is through a glob or explicit.
    pub priority: Priority,
}

/// Identifies a namespace.
#[repr(u8)]
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum NamespaceId {
    Type = 0,
    Symbol = 1,
    Macro = 2,
    Scope = 3,
}

/// A binding priority. Bindings created through globs (`use thing::*`) have lower
/// priority than explicit imports / declarations.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Priority {
    Glob,
    Explicit,
}

/// A prelude. There's one of these per-crate. Each is constructed before crate name resolution
/// begins. They mostly act like a normal scope, but they apply to a whole crate.
///
/// Not to be confused with the "language prelude", i.e. std::prelude::v1 -- that is a set of names
/// that's *added* to each crate's prelude. However, a crate prelude can include other items as well;
/// notably, extern crates, and macros imported with macro_use!.
///
/// The `#[no_implicit_prelude]` disables the entire crate prelude for some module, including extern crates!
/// External crates must be accessed like `::krate` to work in a no_implicit_prelude module.
#[derive(Serialize, Deserialize, Debug)]
pub struct Prelude {
    /// This scope serves as a fallback for all name lookups within a crate.
    ///
    /// (`Priority` and `Visibility` don't matter in this data structure,
    /// we just reuse Scope for convenience.)
    pub scope: Scope,

    /// External crates are added by their name in `Cargo.toml`. using `extern crate a as b` adds
    /// *both* `a` and `b` to this map.
    ///
    /// This is used to look up paths prefixed with `::`.
    pub extern_crates: Map<Ident, CrateId>
}
