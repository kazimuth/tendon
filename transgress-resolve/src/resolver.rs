use transgress_api::paths::{AbsolutePath, UnresolvedPath};

use crate::namespace::{Namespace, Namespaced};
use crate::walker::ModuleImports;
use transgress_api::items::{DeclarativeMacroItem, MacroItem};
use crate::Map;
use transgress_api::idents::Ident;
use transgress_api::items::Receiver::RefSelf;

// https://github.com/rust-lang/rust/tree/master/src/librustc_resolve

pub mod resolvable;

// macro name resolution is affected by order, right?
//
// see: https://danielkeep.github.io/tlborm/book/mbe-min-scoping.html
//
// other stuff:
// see: https://rust-lang.github.io/rustc-guide/name-resolution.html
// see: https://github.com/rust-lang/rust/blob/master/src/librustc_resolve/lib.rs
// see: https://github.com/rust-lang/rfcs/blob/master/text/1560-name-resolution.md (not yet implemented)
// see: https://doc.rust-lang.org/edition-guide/rust-2018/macros/macro-changes.html
//
// note: don't type uses, allow passthrough (actually the better choice anyway)
//
// TODO #[macro_use] imports
// TODO prelude
// TODO: is_safe_for_auto_derive -- trait has no type members
// TODO: handle rust edition
// TODO: distinguish between parse / walk failures and deliberately ignored items

// https://github.com/rust-lang/rustc-guide/blob/master/src/name-resolution.md
// https://doc.rust-lang.org/reference/items/extern-crates.html
// > When naming Rust crates, hyphens are disallowed. However, Cargo packages may make use of them.
// > In such case, when Cargo.toml doesn't specify a crate name, Cargo will transparently replace -
// > with _ (Refer to RFC 940 for more details).

quick_error! {
    #[derive(Debug)]
    pub enum ResolveError {
        PathNotFound(namespace: &'static str, path: AbsolutePath) {
            display("path {:?} not found in {} namespace", path, namespace)
        }
        ResolveFailed(namespace: &'static str, path: UnresolvedPath, module: AbsolutePath) {
            display("failed to resolve path {:?} in module {:?} [{} namespace]", path, module, namespace)
        }
    }
}

// if not absolute:
//     - check first segment for weirdness
//

/*

lazy_static! {
    static ref SUPER: Ident = "super".into();
    static ref CRATE: Ident = "crate".into();
    static ref SELF_MOD: Ident = "self".into();
    static ref SELF_TYPE: Ident = "Self".into();
}

// TODO: pass to strip all non-visible items
// TODO: how to handle `pub` reexports? original items may not be accessible.

/// A resolved path.
pub struct Resolved {
    /// The shallow version of this path; may be a reexport. Guaranteed to be accessible from
    /// outside the crate.
    shallow: AbsolutePath,
    /// The actual path of the originating exported item. May not be accessible from outside the
    /// crate.
    actual: AbsolutePath,
}

/// Attempt to resolve a path, without taking into account local generic parameters.
/// Guarantees the existence of the target item; returns both the path to the target item and the
/// path to an accessible reexport of the target item (since the target item may be in a private
/// module.)
pub fn resolve<I: Namespaced>(
    module: &AbsolutePath,
    path: &UnresolvedPath,
    namespace: &Namespace<I>,
    imports: &Namespace<ModuleImports>
) -> Result<Resolved, ResolveError> {
    let failed = || ResolveError::ResolveFailed(I::namespace(), path.clone(), module.clone());
    let mut path = path.clone();
    let mut module = module.clone();
    if &path.path[0] == &*CRATE {
        path.is_absolute = true;
        path.path.remove(0);
    }
    if &path.path[0] == &*SUPER {
        module.path.pop();
        path.path.remove(0);
    }
    if path.is_absolute {
        module.path.clear();
    }

    panic!()
}

/// Resolve a macro.
/// This will repeatedly follow `use` statements to find the actual path of the macro, unlike
/// `resolve_absolute` which doesn't follow indirection.
pub fn resolve_macro<I: Namespaced>(
    module: &AbsolutePath,
    path: &UnresolvedPath,
    namespace: &Namespace<I>,
    imports: &Namespace<ModuleImports>,
) -> Result<AbsolutePath, ResolveError> {
    unimplemented!()
}
*/
