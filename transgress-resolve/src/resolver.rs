// use namespace::{Namespace, Namespaced};

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

// https://github.com/rust-lang/rustc-guide/blob/master/src/name-resolution.md
// https://doc.rust-lang.org/reference/items/extern-crates.html
// > When naming Rust crates, hyphens are disallowed. However, Cargo packages may make use of them.
// > In such case, when Cargo.toml doesn't specify a crate name, Cargo will transparently replace -
// > with _ (Refer to RFC 940 for more details).

// alg
//     walk paths
//     parse
//     resolve macros (via use thing::macro, macro_use)
//     expand macros
//     parse new data, walk, expand macros until done
//     resolve everything else

/*
fn resolve<I: Namespaced>(
    module: AbsolutePath,
    path: UnresolvedPath,
    namespace: &Namespace<I>,
    imports: &Namespace<ModuleImports>
) -> Result<AbsolutePath, WalkError> {

}
*/
