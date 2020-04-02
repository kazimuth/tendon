use super::{LocationMetadata, WalkError};
use crate::lower::attributes::extract_attribute;
use std::fs;
use std::path::PathBuf;
use syn;
use tendon_api::attributes::{Metadata, Span, Visibility};
use tendon_api::builtins::{CORE_CRATE, ALLOC_CRATE, STD_CRATE, BUILTIN_TYPES};
use tendon_api::crates::CrateData;
use tendon_api::database::{DbView, NamespaceLookup, Namespace};
use tendon_api::identities::{CrateId, Identity};
use tendon_api::paths::{Ident, UnresolvedPath};
use tendon_api::scopes::{Scope, Prelude, Binding, Priority, NamespaceId};
use tendon_api::Map;
use tendon_api::items::{TypeItem, SymbolItem, MacroItem};
use tracing::info;

/// Find the path for a module.
fn find_source_file(parent: &LocationMetadata, item: &syn::ItemMod) -> Result<PathBuf, WalkError> {
    let look_at = if let Some(path) = extract_attribute(&item.attrs, "path") {
        let string = path
            .get_assigned_string()
            .ok_or_else(|| WalkError::MalformedPathAttribute(format!("{:?}", path)))?;
        if string.ends_with(".rs") {
            // TODO are there more places we should check?
            let dir = parent.source_file.parent().ok_or(WalkError::Root)?;
            return Ok(dir.join(string));
        }
        string
    } else {
        format!("{}", item.ident)
    };

    let mut root_normal = parent.crate_data.entry.parent().unwrap().to_owned();
    for entry in &parent.module_path.path {
        root_normal.push(entry.to_string());
    }

    let root_renamed = parent.source_file.parent().unwrap().to_owned();

    for root in [root_normal, root_renamed].iter() {
        let to_try = [
            root.join(format!("{}.rs", look_at)),
            root.join(look_at.clone()).join("mod.rs"),
        ];
        for to_try in to_try.iter() {
            if let Ok(metadata) = fs::metadata(to_try) {
                if metadata.is_file() {
                    return Ok(to_try.clone());
                }
            }
        }
    }

    Err(WalkError::ModuleNotFound)
}

/// Resolve an item in the current crate or the database of dependencies.
///
/// Rules:
/// ~thing~: checks current scope, then other crates, then prelude
/// ~::thing~ looks up ~thing~ as a crate only
/// ~crate::~, ~self::~, and ~super::~ are exact
///
/// Note that this doesn't cover everything, there's special stuff for macros.
///
/// Invariant: every recursive call of this function should reduce the path somehow,
/// by either stripping off a prefix or a module export.
///
/// If this returns `WalkError::NotYetResolved`, that means we haven't found the path yet, and should
/// keep it on the work list.
/// If, however, it returns `WalkError::CannotResolve`
/// that means the path has requested something that doesn't make sense (e.g. a missing item in a
/// dependent crate), so we'll need to throw its containing item out.
/// `WalkError::Impossible` is reserved for syntactic violations, maybe emerging after botched
/// macro transcription.
///
///
fn try_to_resolve<I: NamespaceLookup>(
    db: &mut DbView,
    path: &UnresolvedPath,
    in_module: &Identity,
    orig_module: &Identity,
) -> Result<Identity, WalkError> {
    // TODO refactor output enum
    // TODO this is way out of date

    if path.rooted {
        // look for other crates in crate root
    }

    if path.path.len() > 1 {
        let first = match &*path.path[0] {
            "crate" => unimplemented!(),
            "self" => unimplemented!(),
            "super" => unimplemented!(),
            other => other,
        };
    }

    unimplemented!()

    /*
    let scope =  db.get_item::<Scope>(in_module).ok_or_else(|| if in_module.crate_ == orig_module.crate_ {
        // might not yet be expanded
        WalkError::NotYetResolved
    } else {
        // in the dep graph, crate is complete: will never resolve
        WalkError::CannotResolve
    })?;
    */
}

/// Build up a prelude.
///
/// Note that the extra information on Bindings is ignored within the prelude, we just reuse Scope
/// for convenience.
///
/// Note that the definitions of these items don't live here! it's just a way to link names in the
/// prelude to the correct paths.
///
/// TODO: do these all live in the right place? how do we make sure the identities are correct?
pub fn build_prelude(
    db: &mut DbView,
    crate_data: &CrateData,
    no_std: bool,
    crate_bindings: &Map<Ident, CrateId>,
    macro_use_crates: &Vec<CrateId>,
) -> Result<Prelude, WalkError> {
    let mut scope = Scope::new(Metadata::fake("{prelude}"), false);

    // https://doc.rust-lang.org/1.29.0/book/first-edition/primitive-types.html
    {
        let types = scope.get_bindings_mut::<TypeItem>();
        for (name, builtin) in &*BUILTIN_TYPES {
            types.insert(name.clone(), Binding {
                visibility: Visibility::Pub,
                identity: builtin.clone(),
                priority: Priority::Glob
            });
        }
    }

    #[inline(never)]
    fn add_to_prelude<I: NamespaceLookup>(scope: &mut Scope, crate_: &CrateId, path: &str) {
        let path_ = path.split("::").collect::<Vec<_>>();
        let id = Identity::new(crate_, &path_);
        let binding = Binding {
            // doesn't matter for prelude
            visibility: Visibility::Pub,
            identity: id,
            priority: Priority::Glob
        };
        scope.get_bindings_mut::<I>().insert(path_.iter().last().unwrap().into(), binding);

        if I::namespace_id() == NamespaceId::Type {
            add_to_prelude::<Scope>(scope, crate_, path);
        }
    }
    #[inline(never)]
    fn add_crate(scope: &mut Scope, crate_: &CrateId, name: Ident) {
        let binding = Binding {
            visibility: Visibility::Pub,
            identity: Identity::root(crate_),
            priority: Priority::Glob
        };
        scope.get_bindings_mut::<Scope>().insert(name, binding);
    }

    let core_ = &*CORE_CRATE;
    let alloc_ = &*ALLOC_CRATE;
    let std_ = &*STD_CRATE;

    // add traits
    add_to_prelude::<TypeItem>(&mut scope, core_, "marker::Copy");
    add_to_prelude::<TypeItem>(&mut scope, core_, "marker::Send");
    add_to_prelude::<TypeItem>(&mut scope, core_, "marker::Sized");
    add_to_prelude::<TypeItem>(&mut scope, core_, "marker::Sync");
    add_to_prelude::<TypeItem>(&mut scope, core_, "marker::Unpin");
    add_to_prelude::<TypeItem>(&mut scope, core_, "ops::Drop");
    add_to_prelude::<TypeItem>(&mut scope, core_, "ops::Fn");
    add_to_prelude::<TypeItem>(&mut scope, core_, "ops::FnMut");
    add_to_prelude::<TypeItem>(&mut scope, core_, "ops::FnOnce");
    add_to_prelude::<TypeItem>(&mut scope, core_, "clone::Clone");
    add_to_prelude::<TypeItem>(&mut scope, core_, "cmp::Eq");
    add_to_prelude::<TypeItem>(&mut scope, core_, "cmp::Ord");
    add_to_prelude::<TypeItem>(&mut scope, core_, "cmp::PartialEq");
    add_to_prelude::<TypeItem>(&mut scope, core_, "cmp::PartialOrd");
    add_to_prelude::<TypeItem>(&mut scope, core_, "convert::AsMut");
    add_to_prelude::<TypeItem>(&mut scope, core_, "convert::AsRef");
    add_to_prelude::<TypeItem>(&mut scope, core_, "convert::From");
    add_to_prelude::<TypeItem>(&mut scope, core_, "convert::Into");
    add_to_prelude::<TypeItem>(&mut scope, core_, "default::Default");
    add_to_prelude::<TypeItem>(&mut scope, core_, "iter::DoubleEndedIterator");
    add_to_prelude::<TypeItem>(&mut scope, core_, "iter::ExactSizeIterator");
    add_to_prelude::<TypeItem>(&mut scope, core_, "iter::Extend");
    add_to_prelude::<TypeItem>(&mut scope, core_, "iter::IntoIterator");
    add_to_prelude::<TypeItem>(&mut scope, core_, "iter::Iterator");
    add_to_prelude::<TypeItem>(&mut scope, core_, "option::Option");
    add_to_prelude::<TypeItem>(&mut scope, core_, "result::Result");
    add_to_prelude::<TypeItem>(&mut scope, core_, "hash::macros::Hash;");

    /// add symbols, of which there aren't many.
    add_to_prelude::<SymbolItem>(&mut scope, core_, "mem::drop");
    add_to_prelude::<SymbolItem>(&mut scope, core_, "option::Option::None");
    add_to_prelude::<SymbolItem>(&mut scope, core_, "option::Option::Some");
    add_to_prelude::<SymbolItem>(&mut scope, core_, "result::Result::Err");
    add_to_prelude::<SymbolItem>(&mut scope, core_, "result::Result::Ok");

    /// add macros.
    add_to_prelude::<MacroItem>(&mut scope, core_, "asm");
    add_to_prelude::<MacroItem>(&mut scope, core_, "assert");
    add_to_prelude::<MacroItem>(&mut scope, core_, "cfg");
    add_to_prelude::<MacroItem>(&mut scope, core_, "column");
    add_to_prelude::<MacroItem>(&mut scope, core_, "compile_error");
    add_to_prelude::<MacroItem>(&mut scope, core_, "concat");
    add_to_prelude::<MacroItem>(&mut scope, core_, "concat_idents");
    add_to_prelude::<MacroItem>(&mut scope, core_, "env");
    add_to_prelude::<MacroItem>(&mut scope, core_, "file");
    add_to_prelude::<MacroItem>(&mut scope, core_, "format_args");
    add_to_prelude::<MacroItem>(&mut scope, core_, "format_args_nl");
    add_to_prelude::<MacroItem>(&mut scope, core_, "global_asm");
    add_to_prelude::<MacroItem>(&mut scope, core_, "include");
    add_to_prelude::<MacroItem>(&mut scope, core_, "include_bytes");
    add_to_prelude::<MacroItem>(&mut scope, core_, "include_str");
    add_to_prelude::<MacroItem>(&mut scope, core_, "line");
    add_to_prelude::<MacroItem>(&mut scope, core_, "log_syntax");
    add_to_prelude::<MacroItem>(&mut scope, core_, "module_path");
    add_to_prelude::<MacroItem>(&mut scope, core_, "option_env");
    add_to_prelude::<MacroItem>(&mut scope, core_, "stringify");
    add_to_prelude::<MacroItem>(&mut scope, core_, "trace_macros");
    add_to_prelude::<MacroItem>(&mut scope, core_, "macros::builtin::bench");
    add_to_prelude::<MacroItem>(&mut scope, core_, "macros::builtin::global_allocator");
    add_to_prelude::<MacroItem>(&mut scope, core_, "macros::builtin::test");
    add_to_prelude::<MacroItem>(&mut scope, core_, "macros::builtin::test_case");

    // extra stuff from the std prelude. not present if we're currently in core.
    // we resolve these to alloc cause... that's the truth? idk
    if !no_std && &crate_data.id != &*CORE_CRATE {
        add_to_prelude::<TypeItem>(&mut scope, alloc_, "borrow::ToOwned");
        add_to_prelude::<TypeItem>(&mut scope, alloc_, "boxed::Box");
        add_to_prelude::<TypeItem>(&mut scope, alloc_, "string::String");
        add_to_prelude::<TypeItem>(&mut scope, alloc_, "string::ToString");
        add_to_prelude::<TypeItem>(&mut scope, alloc_, "vec::Vec");
    }

    if no_std {
        // TODO what about renames?
        add_crate(&mut scope, core_, "core".into());
    } else {
        add_crate(&mut scope, std_, "std".into());
    }

    // the extern crate prelude
    for (name, crate_) in crate_bindings {
        add_crate(&mut scope, crate_, name.clone());
    }

    // the #[macro_use] prelude
    for crate_ in macro_use_crates {
        let bindings = scope.get_bindings_mut::<MacroItem>();

        let crate_root = db.get_item::<Scope>(&Identity::root(crate_)).expect("all dependent crates must be resolved");

        // macros are only added to the crate root if they are #[macro_export]
        let exported_macros = crate_root.get_bindings::<MacroItem>();

        for (name, dep_binding) in exported_macros {
            let binding = Binding {
                identity: dep_binding.identity.clone(),
                visibility: Visibility::Pub,
                priority: Priority::Glob,
            };
            if let Some(previous) = bindings.insert(name.clone(), binding) {
                panic!("multiply-defined macro `{}` in prelude: original {:?}, new {:?}", name, previous.identity, dep_binding.identity);
            }
        }
    }

    Ok(Prelude(scope))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tendon_api::database::{Db, ROOT_SCOPE_NAME};
    use tendon_api::identities::{TEST_CRATE_A, TEST_CRATE_B};

    #[test]
    fn prelude() {
        let crate_a = &*TEST_CRATE_A;
        let crate_b = &*TEST_CRATE_B;

        let mut db = Db::fake_db();
        let mut view = db.view_once_per_thread_i_promise();


        let a_root = view.add_root_scope(crate_a.clone(), Scope::new(Metadata::fake((&*ROOT_SCOPE_NAME).clone()), true)).unwrap();
        view.add_binding::<MacroItem>(&a_root,
                         "a_exported_macro".into(), Identity::new(crate_a, &["a_exported_macro"]), Visibility::Pub,
        Priority::Glob).unwrap();


        let mut crate_bindings = Map::default();
        crate_bindings.insert("test_crate_a".into(), (&*TEST_CRATE_A).clone());
        let mut macro_use_crates = Vec::new();
        macro_use_crates.push((&*TEST_CRATE_A).clone());

        let b_prelude = build_prelude(&mut view,
                                      &CrateData::fake((&*TEST_CRATE_B).clone()),
                                      false,
                                      &crate_bindings,
                                      &macro_use_crates
        ).unwrap();

        let prelude_scopes = b_prelude.0.get_bindings::<Scope>();
        assert!(prelude_scopes.contains_key(&Ident::from("test_crate_a")));

        let prelude_macros = b_prelude.0.get_bindings::<MacroItem>();
        assert!(prelude_macros.contains_key(&Ident::from("a_exported_macro")));
    }
}
