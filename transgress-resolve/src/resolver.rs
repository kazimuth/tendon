use transgress_api::paths::{AbsoluteCrate, AbsolutePath, UnresolvedPath};

use crate::namespace::{Namespace, Namespaced};
use crate::walker::ModuleScope;
use crate::Map;
use lazy_static::lazy_static;
use transgress_api::idents::Ident;
use transgress_api::items::Receiver::RefSelf;
use transgress_api::items::{DeclarativeMacroItem, MacroItem};

// https://github.com/rust-lang/rust/tree/master/src/librustc_resolve

pub mod resolvable;

extern crate syn as syn3;

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
// TODO: for traits, `Self` is a generic; for other things, `Self` should resolve to self

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

lazy_static! {
    static ref SUPER: Ident = "super".into();
    static ref CRATE: Ident = "crate".into();
    static ref SELF_MOD: Ident = "self".into();
    static ref SELF_TYPE: Ident = "Self".into();
}

// TODO: pass to strip all non-visible items
// TODO: how to handle `pub` reexports? original items may not be accessible.
// TODO: send + sync determination w/ conservative failure for unresolved contents
// TODO: inject prelude
// TODO: inject primitives
// TODO: add all local definitions to `ModuleScope`

// TODO: how does `use` name resolution work??
// https://doc.rust-lang.org/edition-guide/rust-2018/module-system/path-clarity.html
// https://github.com/rust-lang/rust/blob/1064d41/src/librustc_resolve/resolve_imports.rs
//     NameBinding: https://github.com/rust-lang/rust/blob/1064d41/src/librustc_resolve/lib.rs#L563
//     import resolution: https://github.com/rust-lang/rust/blob/1064d41/src/librustc_resolve/resolve_imports.rs#L179-L416
//         runs name resolution per-namespace
//         ident imports over-shadow globs
//         "blacklisting"? for wrong namespaces maybe?
// compare: https://github.com/thepowersgang/mrustc/blob/master/src/resolve/use.cpp ; simpler codebase
//     just converts to absolute path
// how do `extern crate` statements affect this?
//     by testing: at root, `extern crate` affects paths used by `use` statements; at other locations,
//     just behaves like a normal `use` statement. That is:
//
//     // (crate root)
//     extern crate syn as syn2;
//     mod p {
//         use syn; // valid
//         use syn2; // valid
//         extern crate syn as syn3; // valid
//         mod q {
//             use syn; // valid
//             use syn2; // valid
//             // use syn3; //invalid!
//             use super::syn3; // ...but this is valid
//
//         }
//     }
//
// also, `extern crate` statements at crate root will overwrite other dependencies in the `use`
// namespace

// resolution priority:
//      explicit `use` / local definition
//      glob import
//

// TODO: choose "canonical import" for every defined import
//     prefer short paths

// TODO: fall back to non-pub paths in globs in case of lookup failure
//     paper over shadowing issues...

/*

// https://github.com/rust-lang/rust/blob/1064d41/src/librustc_resolve/macros.rs
// https://github.com/rust-lang/rust/blob/1064d41/src/librustc_resolve/late.rs

/// Attempt to resolve a path, following `use` and `pub use` imports. Will give a path to a definition.
/// Resulting path may not be accessible outside its defining crate.
pub fn resolve_recursive<I: Namespaced>(
    module: &AbsolutePath,
    path: &UnresolvedPath,
    namespace: &Namespace<I>,
    imports: &Namespace<ModuleScope>
) -> Result<AbsolutePath, ResolveError> {
    let mut path = path.clone();
    let mut module = module.clone();
    // whether to check in the current crate:
    let mut check_crate = true;
    // whether to check in external crates:
    let mut check_external = true;

    if &path.path[0] == &*CRATE {
        check_external = false;
        path.path.remove(0);
    } else if &path.path[0] == &*SUPER {
        check_external = false;
        module.path.pop();
        path.path.remove(0);
    } else if path.is_absolute {
        check_crate = false;
    }

    let imported = imports.inspect(&module, |imports| {
        if imports.pub_imports.contains_key()
    });

    if check_crate {
        // TODO: how much validation is needed here?
        let root_module_exists = imports.contains(&AbsolutePath { crate_: module.crate_.clone(), path: vec![path.path[0].clone()]});

        if root_module_exists {
            return Ok(AbsolutePath { crate_: module.crate_.clone(), path: path.path.clone()})
        }
    }
    if check_external {

    }

    Err(ResolveError::ResolveFailed(I::namespace(), path.clone(), module.clone()))
}

pub fn resolve_imports(
    imports: ModuleScope,
    module: &AbsolutePath,

) -> ModuleScope {

}
*/
