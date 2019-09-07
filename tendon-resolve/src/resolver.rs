use tendon_api::paths::{AbsolutePath, UnresolvedPath};

use crate::namespace::Namespace;
use crate::tools::CrateData;
use crate::tools::RustEdition;
use crate::walker::ModuleScope;
use lazy_static::lazy_static;
use tendon_api::idents::Ident;
use tendon_api::items::ModuleItem;
use tendon_api::paths::Path;

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
        ResolveMaybeFailed {
            display("resolve may have failed for path")
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
// TODO: rust 2015 support

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
//         use syn;
//         use syn2;
//         extern crate syn as syn3; // valid
//         mod q {
//             use syn;
//             use syn2;
//             // use syn3; // invalid!
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

// https://github.com/rust-lang/rust/blob/1064d41/src/librustc_resolve/macros.rs
// https://github.com/rust-lang/rust/blob/1064d41/src/librustc_resolve/late.rs

/*
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
*/

/// Convert a `use`d path to an absolute path, rust 2018 rules.
/// - if a local module was declared in this scope, it overrides extern crates
/// - crate-local modules that are children of the root are *not* looked up in a `use` statement
/// Returns whether the path has been resolved yet (this operation might leave it unchanged, if
/// e.g. it refers to an unexpanded macro.)
pub fn absolutize_use_2018(
    path: &mut Path,
    module: &AbsolutePath,
    modules: &Namespace<ModuleItem>,
    crate_data: &CrateData,
) -> bool {
    let mut unresolved = if let Path::Unresolved(unresolved) = path {
        unresolved.clone()
    } else {
        return true;
    };
    let mut module = module.clone();
    let ident = unresolved.path.remove(0);

    if &ident == &*CRATE {
        module.path.clear();
        module.path.append(&mut unresolved.path);
        *path = Path::Absolute(module);
        return true;
    } else if &ident == &*SUPER {
        module.path.pop();
        module.path.append(&mut unresolved.path);
        *path = Path::Absolute(module);
        return true;
    }

    // is the `use` a submodule?
    let mut possible = module.clone().join(ident.clone());
    if !unresolved.is_absolute && modules.contains(&possible) {
        possible.path.append(&mut unresolved.path);
        *path = Path::Absolute(possible);
        return true;
    }

    if let Some(crate_) = crate_data.deps.get(&ident) {
        *path = Path::Absolute(AbsolutePath::new(crate_.clone(), unresolved.path));
        return true;
    }

    // Path not found.
    // It may be the result of macro expansion...
    return false;
}

/// Attempt to resolve all `use` statements in a module scope.
/// Uses Rust 2018 rules.
fn absolutize_imports_2018(
    scope: &mut ModuleScope,
    module: &AbsolutePath,
    modules: &Namespace<ModuleItem>,
    crate_data: &CrateData,
) -> bool {
    let &mut ModuleScope {
        ref mut imports,
        ref mut pub_imports,
        ref mut glob_imports,
        ref mut pub_glob_imports,
    } = scope;
    let mut all_success = true;
    for path in imports
        .values_mut()
        .chain(pub_imports.values_mut())
        .chain(glob_imports.iter_mut())
        .chain(pub_glob_imports.iter_mut())
    {
        let success = absolutize_use_2018(path, module, modules, crate_data);
        all_success = all_success && success;
    }
    all_success
}

pub fn absolutize_imports(
    scope: &mut ModuleScope,
    module: &AbsolutePath,
    modules: &Namespace<ModuleItem>,
    crate_data: &CrateData,
) -> bool {
    match crate_data.rust_edition {
        RustEdition::Rust2018 => absolutize_imports_2018(scope, module, modules, crate_data),
        // TODO: WRONG! implement this!
        RustEdition::Rust2015 => absolutize_imports_2018(scope, module, modules, crate_data),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Map;
    use tendon_api::attributes::{Metadata, Span, Visibility};
    use tendon_api::paths::AbsoluteCrate;

    #[test]
    fn imports_2018() {
        spoor::init();

        let fake_module = ModuleItem {
            metadata: Metadata {
                visibility: Visibility::Pub,
                docs: None,
                must_use: None,
                deprecated: None,
                extra_attributes: vec![],
                span: Span {
                    source_file: "fake_file.rs".into(),
                    start_line: 0,
                    start_column: 0,
                    end_line: 0,
                    end_column: 0,
                },
            },
            name: "fake_module".into(),
        };

        let crate_1 = AbsoluteCrate::new("crate_1", "0.0.0");
        let crate_2 = AbsoluteCrate::new("crate_2", "0.0.0");

        let module = AbsolutePath::new(crate_1.clone(), &["module"]);
        let submodule = AbsolutePath::new(crate_1.clone(), &["module", "submodule"]);
        let module2 = AbsolutePath::new(crate_1.clone(), &["module2"]);

        let modules = Namespace::new();
        modules.insert(module.clone(), fake_module.clone()).unwrap();
        modules
            .insert(submodule.clone(), fake_module.clone())
            .unwrap();
        modules
            .insert(module2.clone(), fake_module.clone())
            .unwrap();

        let mut crate_data = CrateData {
            crate_: crate_1.clone(),
            deps: Map::default(),
            features: vec![],
            manifest_path: "".into(),
            entry: "".into(),
            is_proc_macro: false,
            rust_edition: RustEdition::Rust2018,
        };
        crate_data.deps.insert("crate_2".into(), crate_2.clone());

        let mut scope = ModuleScope::new();
        scope.glob_imports.push(Path::fake("crate_2::thing"));
        scope.glob_imports.push(Path::fake("submodule::thing"));
        scope.glob_imports.push(Path::fake("crate::module2::thing"));

        assert!(absolutize_imports(
            &mut scope,
            &module,
            &modules,
            &crate_data
        ));
        assert_match!(scope.glob_imports[0], Path::Absolute(abs) => {
            assert_eq!(abs, &AbsolutePath::new(crate_2, &["thing"]));
        });
        assert_match!(scope.glob_imports[1], Path::Absolute(abs) => {
            assert_eq!(abs, &submodule.clone().join("thing"));
        });
        assert_match!(scope.glob_imports[2], Path::Absolute(abs) => {
            assert_eq!(abs, &module2.clone().join("thing"));
        });
    }
}
