use super::{LocationMetadata, WalkError};
use crate::lower::attributes::extract_attribute;
use std::fs;
use std::path::PathBuf;
use syn;
use tendon_api::attributes::{Metadata, Span};
use tendon_api::builtins::CORE_CRATE;
use tendon_api::crates::CrateData;
use tendon_api::database::{DbView, NamespaceLookup};
use tendon_api::identities::{CrateId, Identity};
use tendon_api::paths::{Ident, UnresolvedPath};
use tendon_api::scopes::Scope;
use tendon_api::Map;

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

// TODO refactor output enum

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
fn build_prelude(
    db: &mut DbView,
    crate_data: &CrateData,
    no_std: bool,
    crate_bindings: Map<Ident, CrateId>,
    macro_use_crates: Vec<CrateId>,
) -> Result<Scope, WalkError> {
    let mut result = Scope::new(Metadata::fake("{prelude}"), false);

    // first, the language-level prelude, always added.
    // https://doc.rust-lang.org/1.29.0/book/first-edition/primitive-types.html

    unimplemented!()
}
