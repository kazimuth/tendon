//! Scopes and bindings.

use crate::attributes::{Metadata, Visibility};
use crate::database::NamespaceLookup;
use crate::identities::Identity;
use crate::paths::Ident;
use crate::scopes::Priority::{Explicit, Glob};
use crate::Map;
use hashbrown::hash_map::Entry;
use serde::{Deserialize, Serialize};
use std::fmt;
use tracing::error;

/// A scope, containing bindings for all 4 namespaces.
///
/// Scopes may be named (modules, enums, traits) or anonymous (impl blocks, function bodies.)
/// For non-module scopes, most metadata is only stored on the item, although the span and name is stored
/// here too.
///
/// Also, we're cheeky and put lifetimes in the type namespace, but their names have ticks so
/// its basically distinct.
#[derive(Serialize, Deserialize)]
pub struct Scope {
    /// Metadata on a scope (e.g. module doc commments)
    pub metadata: Metadata,
    /// If this is a module or something else.
    pub is_module: bool,
    /// Bindings
    bindings: [Map<Ident, Binding>; 4],
}
impl Scope {
    /// Create a new module scope.
    pub fn new(metadata: Metadata, is_module: bool) -> Scope {
        Scope {
            metadata,
            is_module,
            bindings: Default::default(),
        }
    }

    /// Get a binding by namespace id. Does NOT check inherited scopes or prelude.
    pub fn get_by(&self, namespace_id: NamespaceId, ident: &Ident) -> Option<&Binding> {
        self.bindings[namespace_id as usize].get(ident)
    }

    /// Insert a binding by namespace id. Returns Err if already present.
    /// Does NOT update back links!
    pub fn insert_by(
        &mut self,
        namespace_id: NamespaceId,
        ident: Ident,
        target: Identity,
        visibility: Visibility,
        priority: Priority,
    ) -> Result<(), ()> {
        match self.bindings[namespace_id as usize].entry(ident) {
            Entry::Occupied(occ) => {
                let binding = occ.get();
                match (binding.priority, priority) {
                    (Glob, _) | (Explicit, Explicit) => {
                        error!("binding priority inversion?");
                        Err(())
                    }
                    (Explicit, Glob) => Ok(()),
                }
            }
            Entry::Vacant(vac) => {
                vac.insert(Binding {
                    identity: target,
                    visibility,
                    priority,
                });
                Ok(())
            }
        }
    }

    /// Iterate bindings by namespace id.
    pub fn iter_by(&self, namespace_id: NamespaceId) -> impl Iterator<Item = (&Ident, &Binding)> {
        self.bindings[namespace_id as usize].iter()
    }

    /// Get a binding in a namespace. Does NOT check inherited scopes or prelude.
    pub fn get<I: NamespaceLookup>(&self, ident: &Ident) -> Option<&Binding> {
        self.get_by(I::namespace_id(), ident)
    }

    /// Insert a binding in a namespace. Returns Err if already present.
    /// Does NOT update back links!
    pub fn insert<I: NamespaceLookup>(
        &mut self,
        ident: Ident,
        target: Identity,
        visibility: Visibility,
        priority: Priority,
    ) -> Result<(), ()> {
        self.insert_by(I::namespace_id(), ident, target, visibility, priority)
    }

    /// Iterate bindings in a namespace.
    pub fn iter<I: NamespaceLookup>(&self) -> impl Iterator<Item = (&Ident, &Binding)> {
        self.iter_by(I::namespace_id())
    }
}
impl fmt::Debug for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        let mut s = f.debug_struct("Scope");
        s.field("metadata", &self.metadata);
        s.field("is_module", &self.is_module);

        let get_name = |id: NamespaceId| match id {
            NamespaceId::Type => "types",
            NamespaceId::Macro => "macros",
            NamespaceId::Symbol => "symbols",
            NamespaceId::Scope => "scopes",
        };

        for (bindings, ns) in self.bindings.iter().zip(NamespaceId::values().iter()) {
            if bindings.len() > 0 {
                s.field(get_name(*ns), bindings);
            }
        }

        s.finish()
    }
}

/// A name binding. (Nothing to do with the idea of "language bindings".)
#[derive(Serialize, Deserialize, Debug)]
pub struct Binding {
    /// The final target this bind | (Glob, Glob)ing points to. If this points to another binding
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
impl NamespaceId {
    /// Invariant: will always be in integer order.
    pub fn values() -> [NamespaceId; 4] {
        [
            NamespaceId::Type,
            NamespaceId::Symbol,
            NamespaceId::Macro,
            NamespaceId::Scope,
        ]
    }
}

/// A binding priority. Bindings created through globs (`use thing::*`) have lower
/// priority than explicit imports / declarations.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Priority {
    Glob,
    Explicit,
}
