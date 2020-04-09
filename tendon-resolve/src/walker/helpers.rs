use super::{LocationMetadata, WalkError};
use crate::lower::attributes::extract_attribute;
use std::borrow::Cow;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use syn;
use tendon_api::attributes::Visibility;
use tendon_api::builtins::{ALLOC_CRATE, BUILTIN_TYPES, CORE_CRATE};
use tendon_api::database::{Crate, Db, NamespaceLookup};
use tendon_api::identities::{CrateId, Identity};
use tendon_api::items::{MacroItem, SymbolItem, TypeItem};
use tendon_api::paths::{Ident, UnresolvedPath};
use tendon_api::scopes::{NamespaceId, Priority, Scope};
use tracing::error;

/// Resolve an item in the current crate or the database of dependencies.
///
/// Rules:
/// ~thing~: checks current scope, then prelude
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
pub(crate) fn try_to_resolve(
    db: &Db,
    crate_in_progress: &Crate,
    in_module: &Identity,
    namespace: NamespaceId,
    path: &UnresolvedPath,
) -> Result<Identity, ResolveError> {
    // use a Cow to do fewer allocations
    let path = ResolvingPath {
        path: Cow::from(&path.path[..]),
        rooted: path.rooted,
    };

    try_to_resolve_rec(
        db,
        crate_in_progress,
        in_module,
        in_module,
        namespace,
        true,
        path,
    )
}

fn try_to_resolve_rec(
    db: &Db,
    crate_in_progress: &Crate,
    orig_module: &Identity,
    in_module: &Identity,
    namespace_id: NamespaceId,
    check_prelude: bool,
    path: ResolvingPath,
) -> Result<Identity, ResolveError> {
    if path.path.len() == 0 {
        error!("impossible path? {:?}", path);
        return Err(ResolveError::Impossible);
    }

    let get_crate = |id: &CrateId| -> &Crate {
        if id == &crate_in_progress.id {
            &crate_in_progress
        } else {
            db.get_crate(id)
        }
    };

    let in_crate = get_crate(&in_module.crate_);

    if path.rooted {
        // `::something`
        // look for other crates in crate root
        let target_crate = in_crate
            .extern_crate_bindings
            .get(&path.path[0])
            .ok_or_else(|| {
                error!(
                    "no dependency `{}` in crate {:?}",
                    &path.path[0], in_crate.id
                );
                ResolveError::Impossible
            })?;

        let target_module = Identity::root(target_crate);

        let new_path = ResolvingPath {
            path: Cow::from(&path.path[1..]),
            rooted: false,
        };
        return try_to_resolve_rec(
            db,
            crate_in_progress,
            orig_module,
            &target_module,
            namespace_id,
            false,
            new_path,
        );
    }

    let get_binding_by = |namespace_id, ident| -> Result<Identity, ResolveError> {
        let binding = in_crate
            .get_binding_by(in_module, namespace_id, ident)
            .or_else(|| {
                if check_prelude {
                    in_crate.prelude.get_by(namespace_id, ident)
                } else {
                    None
                }
            })
            .ok_or(ResolveError::Pending)?;

        if binding.visibility.is_visible_in(orig_module) {
            Ok(binding.identity.clone())
        } else {
            Err(ResolveError::Pending)
        }
    };

    if path.path.len() == 1 {
        // 1 segment left!
        let ident = &path.path[0];

        get_binding_by(namespace_id, ident)
    } else {
        // path longer than 1
        let target_module = match &*path.path[0] {
            "crate" => Identity::root(&in_module.crate_),
            "self" => in_module.clone(),
            "super" => in_module.parent().ok_or_else(|| {
                error!("no parent of {:?}", in_module);
                ResolveError::Impossible
            })?,
            _ => {
                let seg = &path.path[0];
                get_binding_by(NamespaceId::Scope, seg)?
            }
        };

        let remaining = ResolvingPath {
            path: Cow::from(&path.path[1..]),
            rooted: false,
        };

        try_to_resolve_rec(
            db,
            crate_in_progress,
            orig_module,
            &target_module,
            namespace_id,
            false,
            remaining,
        )
    }
}

#[derive(Clone)]
struct ResolvingPath<'a> {
    path: Cow<'a, [Ident]>,
    rooted: bool,
}
impl<'a> fmt::Debug for ResolvingPath<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, seg) in self.path.iter().enumerate() {
            if i > 0 || self.rooted {
                f.write_str("::")?;
            }
            f.write_str(&seg)?;
        }
        Ok(())
    }
}

quick_error! {
/// Possible outcomes of resolving a path.
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub enum ResolveError {
        /// A name that will never be resolved due to issues in `tendon`;
        /// it and anything that depends on it should be abandoned.
        Impossible {
            display("resolution impossible")
        }
        /// A name that has not yet been resolved.
        Pending {
            display("resolution pending")
        }
    }
}

/// Add a crate dep, not necessarily with an import statement.
/// Adds only to prelude/extern_crate_bindings, doesn't add to root scope.
pub fn add_crate_dep(
    crate_: &mut Crate,
    extern_crate_id: &CrateId,
    name: Ident,
) -> Result<(), WalkError> {
    if let Some(_) = crate_
        .extern_crate_bindings
        .insert(name.clone(), extern_crate_id.clone())
    {
        panic!("can't add crate dep twice!");
    }
    crate_.add_prelude_binding_by(NamespaceId::Scope, name, Identity::root(extern_crate_id))?;
    Ok(())
}

/// Add an `extern crate` statement. The extern crate should already be in the Db, and the current
/// crate's root scope should exist.
///
/// Handles importing `#[macro_use]` macros, which are added to the *prelude* (not the textual scopes!).
///
/// See also `add_crate_dep`.
///
/// Subtle effects:
/// ```no_build
/// pub extern crate core as core_;
/// //                       ^ `name`
//
//  pub use crate::core as corea; // fails: undefined in root
//  pub use crate::core_ as coreb; // works
//  pub use core as corec; // works (crate dep)
//  pub use ::core as cored; // works (crate dep)
//  pub use ::core_ as coref; // works
// ```
pub fn add_root_extern_crate(
    db: &Db,
    crate_: &mut Crate,
    extern_crate_id: &CrateId,
    name: &Ident,
    visibility: Visibility,
    macro_use: bool,
) -> Result<(), WalkError> {
    let extern_crate_root_id = Identity::root(extern_crate_id);

    if !crate_.extern_crate_bindings.contains_key(name) {
        add_crate_dep(crate_, extern_crate_id, name.clone())?;
    }

    // add dep as `crate::dep`
    crate_.add_binding::<Scope>(
        &Identity::root(&crate_.id),
        name.clone(),
        extern_crate_root_id.clone(),
        visibility,
        Priority::Explicit,
    )?;

    let extern_crate = db.get_crate(extern_crate_id);
    let crate_root = extern_crate
        .get::<Scope>(&extern_crate_root_id)
        .ok_or(WalkError::ModuleNotFound)?;

    if macro_use {
        // add all macros to prelude!
        for (name, dep_binding) in crate_root.iter::<MacroItem>() {
            crate_.add_prelude_binding_by(
                NamespaceId::Macro,
                name.clone(),
                dep_binding.identity.clone(),
            )?;
        }
    }

    Ok(())
}

/// Add standard entries to a crate prelude.
///
/// Note that the extra information on Bindings is ignored within the prelude, we just reuse Scope
/// for convenience.
///
/// Note that the definitions of these items don't live here! it's just a way to link names in the
/// prelude to the correct paths.
///
/// TODO: do these all live in the right place? how do we make sure the identities are correct?
pub fn add_std_prelude(crate_: &mut Crate, no_std: bool) -> Result<(), WalkError> {
    for (name, builtin) in &*BUILTIN_TYPES {
        crate_.add_prelude_binding_by(NamespaceId::Type, name.clone(), builtin.clone())?;
    }

    #[inline(never)]
    fn add_to_prelude<I: NamespaceLookup>(
        crate_: &mut Crate,
        crate_id: &CrateId,
        path: &str,
    ) -> Result<(), WalkError> {
        let path_ = path.split("::").collect::<Vec<_>>();
        let last: Ident = path_.iter().last().unwrap().into();
        let identity = Identity::new(&crate_id, &path_);

        if I::namespace_id() == NamespaceId::Type {
            add_to_prelude::<Scope>(crate_, crate_id, path)?;
        }

        crate_.add_prelude_binding_by(I::namespace_id(), last, identity)?;

        Ok(())
    }

    let core_ = &*CORE_CRATE;
    let alloc_ = &*ALLOC_CRATE;
    //let std_ = &*STD_CRATE;

    // add traits
    add_to_prelude::<TypeItem>(crate_, core_, "marker::Copy")?;
    add_to_prelude::<TypeItem>(crate_, core_, "marker::Send")?;
    add_to_prelude::<TypeItem>(crate_, core_, "marker::Sized")?;
    add_to_prelude::<TypeItem>(crate_, core_, "marker::Sync")?;
    add_to_prelude::<TypeItem>(crate_, core_, "marker::Unpin")?;
    add_to_prelude::<TypeItem>(crate_, core_, "ops::Drop")?;
    add_to_prelude::<TypeItem>(crate_, core_, "ops::Fn")?;
    add_to_prelude::<TypeItem>(crate_, core_, "ops::FnMut")?;
    add_to_prelude::<TypeItem>(crate_, core_, "ops::FnOnce")?;
    add_to_prelude::<TypeItem>(crate_, core_, "clone::Clone")?;
    add_to_prelude::<TypeItem>(crate_, core_, "cmp::Eq")?;
    add_to_prelude::<TypeItem>(crate_, core_, "cmp::Ord")?;
    add_to_prelude::<TypeItem>(crate_, core_, "cmp::PartialEq")?;
    add_to_prelude::<TypeItem>(crate_, core_, "cmp::PartialOrd")?;
    add_to_prelude::<TypeItem>(crate_, core_, "convert::AsMut")?;
    add_to_prelude::<TypeItem>(crate_, core_, "convert::AsRef")?;
    add_to_prelude::<TypeItem>(crate_, core_, "convert::From")?;
    add_to_prelude::<TypeItem>(crate_, core_, "convert::Into")?;
    add_to_prelude::<TypeItem>(crate_, core_, "default::Default")?;
    add_to_prelude::<TypeItem>(crate_, core_, "iter::DoubleEndedIterator")?;
    add_to_prelude::<TypeItem>(crate_, core_, "iter::ExactSizeIterator")?;
    add_to_prelude::<TypeItem>(crate_, core_, "iter::Extend")?;
    add_to_prelude::<TypeItem>(crate_, core_, "iter::IntoIterator")?;
    add_to_prelude::<TypeItem>(crate_, core_, "iter::Iterator")?;
    add_to_prelude::<TypeItem>(crate_, core_, "option::Option")?;
    add_to_prelude::<TypeItem>(crate_, core_, "result::Result")?;
    add_to_prelude::<TypeItem>(crate_, core_, "hash::macros::Hash;")?;

    // add symbols, of which there aren't many.
    add_to_prelude::<SymbolItem>(crate_, core_, "mem::drop")?;
    add_to_prelude::<SymbolItem>(crate_, core_, "option::Option::None")?;
    add_to_prelude::<SymbolItem>(crate_, core_, "option::Option::Some")?;
    add_to_prelude::<SymbolItem>(crate_, core_, "result::Result::Err")?;
    add_to_prelude::<SymbolItem>(crate_, core_, "result::Result::Ok")?;

    // add macros.
    add_to_prelude::<MacroItem>(crate_, core_, "asm")?;
    add_to_prelude::<MacroItem>(crate_, core_, "assert")?;
    add_to_prelude::<MacroItem>(crate_, core_, "cfg")?;
    add_to_prelude::<MacroItem>(crate_, core_, "column")?;
    add_to_prelude::<MacroItem>(crate_, core_, "compile_error")?;
    add_to_prelude::<MacroItem>(crate_, core_, "concat")?;
    add_to_prelude::<MacroItem>(crate_, core_, "concat_idents")?;
    add_to_prelude::<MacroItem>(crate_, core_, "env")?;
    add_to_prelude::<MacroItem>(crate_, core_, "file")?;
    add_to_prelude::<MacroItem>(crate_, core_, "format_args")?;
    add_to_prelude::<MacroItem>(crate_, core_, "format_args_nl")?;
    add_to_prelude::<MacroItem>(crate_, core_, "global_asm")?;
    add_to_prelude::<MacroItem>(crate_, core_, "include")?;
    add_to_prelude::<MacroItem>(crate_, core_, "include_bytes")?;
    add_to_prelude::<MacroItem>(crate_, core_, "include_str")?;
    add_to_prelude::<MacroItem>(crate_, core_, "line")?;
    add_to_prelude::<MacroItem>(crate_, core_, "log_syntax")?;
    add_to_prelude::<MacroItem>(crate_, core_, "module_path")?;
    add_to_prelude::<MacroItem>(crate_, core_, "option_env")?;
    add_to_prelude::<MacroItem>(crate_, core_, "stringify")?;
    add_to_prelude::<MacroItem>(crate_, core_, "trace_macros")?;
    add_to_prelude::<MacroItem>(crate_, core_, "macros::builtin::bench")?;
    add_to_prelude::<MacroItem>(crate_, core_, "macros::builtin::global_allocator")?;
    add_to_prelude::<MacroItem>(crate_, core_, "macros::builtin::test")?;
    add_to_prelude::<MacroItem>(crate_, core_, "macros::builtin::test_case")?;

    // extra stuff from the std prelude. not present if we're currently in core.
    // we resolve these to alloc cause... that's the truth? idk
    if !no_std && &crate_.id != &*CORE_CRATE {
        add_to_prelude::<TypeItem>(crate_, alloc_, "borrow::ToOwned")?;
        add_to_prelude::<TypeItem>(crate_, alloc_, "boxed::Box")?;
        add_to_prelude::<TypeItem>(crate_, alloc_, "string::String")?;
        add_to_prelude::<TypeItem>(crate_, alloc_, "string::ToString")?;
        add_to_prelude::<TypeItem>(crate_, alloc_, "vec::Vec")?;
    }

    Ok(())
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use tendon_api::attributes::{Metadata, TypeMetadata};
    use tendon_api::crates::CrateData;
    use tendon_api::database::Db;
    use tendon_api::identities::{TEST_CRATE_A, TEST_CRATE_B, TEST_CRATE_C};
    use tendon_api::items::{EnumItem, GenericParams};

    fn fake_type(name: &str) -> TypeItem {
        TypeItem::Enum(EnumItem {
            metadata: Metadata::fake(name),
            type_metadata: TypeMetadata::default(),
            generic_params: GenericParams::default(),
            variants: vec![],
        })
    }

    #[test]
    fn resolve_current() {
        let db = Db::fake_db();

        let test_crate_a = (*TEST_CRATE_A).clone();

        let mut crate_ = Crate::new(test_crate_a.clone());
        let root = crate_.add_root_scope(Metadata::fake("{root}")).unwrap();
        let mod_a = crate_
            .add(&root, Scope::new(Metadata::fake("a"), true))
            .unwrap();
        let mod_a_b = crate_
            .add(&mod_a, Scope::new(Metadata::fake("b"), true))
            .unwrap();
        let type_a_b_c = crate_.add(&mod_a_b, fake_type("C")).unwrap();

        crate_
            .add_binding::<TypeItem>(
                &root,
                "CRenamed".into(),
                type_a_b_c.clone(),
                Visibility::InScope(root.clone()),
                Priority::Explicit,
            )
            .unwrap();
        crate_
            .add_binding::<TypeItem>(
                &mod_a,
                "CLimited".into(),
                type_a_b_c.clone(),
                Visibility::InScope(mod_a.clone()),
                Priority::Explicit,
            )
            .unwrap();

        let root_abc = try_to_resolve(
            &db,
            &crate_,
            &root,
            NamespaceId::Type,
            &UnresolvedPath::fake("a::b::C"),
        )
        .unwrap();
        assert_eq!(root_abc, type_a_b_c);

        let ab_sscr = try_to_resolve(
            &db,
            &crate_,
            &mod_a_b,
            NamespaceId::Type,
            &UnresolvedPath::fake("super::super::CRenamed"),
        )
        .unwrap();
        assert_eq!(ab_sscr, type_a_b_c);

        let invisible = try_to_resolve(
            &db,
            &crate_,
            &root,
            NamespaceId::Type,
            &UnresolvedPath::fake("a::CLimited"),
        );
        assert_eq!(invisible, Err::<Identity, _>(ResolveError::Pending));
    }

    #[test]
    fn extern_crate() {
        let test_crate_a = (*TEST_CRATE_A).clone();
        let test_crate_b = (*TEST_CRATE_B).clone();
        let test_crate_c = (*TEST_CRATE_C).clone();

        let db = Db::fake_db();

        let mut crate_a = Crate::new(test_crate_a.clone());
        let a_root = crate_a.add_root_scope(Metadata::fake("{root}")).unwrap();
        crate_a
            .add_binding::<TypeItem>(
                &a_root,
                "A".into(),
                Identity::new(&test_crate_a, &["A"]),
                Visibility::Pub,
                Priority::Explicit,
            )
            .unwrap();
        db.insert_crate(crate_a);

        let mut crate_b = Crate::new(test_crate_b.clone());
        let b_root = crate_b.add_root_scope(Metadata::fake("{root}")).unwrap();
        crate_b
            .add_binding::<TypeItem>(
                &b_root,
                "B".into(),
                Identity::new(&test_crate_a, &["B"]),
                Visibility::Pub,
                Priority::Explicit,
            )
            .unwrap();
        db.insert_crate(crate_b);

        let mut crate_c = Crate::new(test_crate_c.clone());
        let c_root = crate_c.add_root_scope(Metadata::fake("{root}")).unwrap();
        let c_other = crate_c
            .add(&c_root, Scope::new(Metadata::fake("other"), true))
            .unwrap();

        let a_name: Ident = "a".into();
        let b_name: Ident = "b".into();
        let b_name_alt: Ident = "b_alt".into();

        // in cargo.toml: test_crate_a renamed "a", test_crate_b renamed "b"
        // in root: `extern crate b as b_alt`
        add_crate_dep(&mut crate_c, &test_crate_a, a_name.clone()).unwrap();
        add_crate_dep(&mut crate_c, &test_crate_b, b_name.clone()).unwrap();
        add_root_extern_crate(
            &db,
            &mut crate_c,
            &test_crate_b,
            &b_name_alt.clone(),
            Visibility::Pub,
            false,
        )
        .unwrap();

        // ::a, ::b, ::b_alt exist
        assert!(crate_c.extern_crate_bindings.get(&a_name).is_some());
        assert!(crate_c.extern_crate_bindings.get(&b_name).is_some());
        assert!(crate_c.extern_crate_bindings.get(&b_name_alt).is_some());

        // a, b, b_alt in prelude
        assert!(crate_c.prelude.get::<Scope>(&a_name).is_some());
        assert!(crate_c.prelude.get::<Scope>(&b_name).is_some());
        assert!(crate_c.prelude.get::<Scope>(&b_name_alt).is_some());

        // b_alt in crate root (only, others aren't brought in)
        // don't use `crate_c.get_binding` 'cause it falls back to prelude :^)
        let c_root_scope = crate_c.get::<Scope>(&c_root).unwrap();
        assert!(c_root_scope.get::<Scope>(&a_name).is_none());
        assert!(c_root_scope.get::<Scope>(&b_name).is_none());
        assert!(c_root_scope.get::<Scope>(&b_name_alt).is_some());
        assert!(
            &crate_c
                .get_binding::<Scope>(&c_root, &b_name_alt)
                .unwrap()
                .visibility
                == &Visibility::Pub
        );

        // same checks w/ try_to_resolve
        assert!(try_to_resolve(
            &db,
            &crate_c,
            &c_other,
            NamespaceId::Type,
            &UnresolvedPath::fake("::a::A")
        )
        .is_ok());
        assert!(try_to_resolve(
            &db,
            &crate_c,
            &c_other,
            NamespaceId::Type,
            &UnresolvedPath::fake("::b::B")
        )
        .is_ok());
        assert!(try_to_resolve(
            &db,
            &crate_c,
            &c_other,
            NamespaceId::Type,
            &UnresolvedPath::fake("::b_alt::B")
        )
        .is_ok());

        assert!(try_to_resolve(
            &db,
            &crate_c,
            &c_other,
            NamespaceId::Type,
            &UnresolvedPath::fake("a::A")
        )
        .is_ok());
        assert!(try_to_resolve(
            &db,
            &crate_c,
            &c_other,
            NamespaceId::Type,
            &UnresolvedPath::fake("b::B")
        )
        .is_ok());
        assert!(try_to_resolve(
            &db,
            &crate_c,
            &c_other,
            NamespaceId::Type,
            &UnresolvedPath::fake("b_alt::B")
        )
        .is_ok());

        assert!(try_to_resolve(
            &db,
            &crate_c,
            &c_other,
            NamespaceId::Type,
            &UnresolvedPath::fake("crate::a::A")
        )
        .is_err());
        assert!(try_to_resolve(
            &db,
            &crate_c,
            &c_other,
            NamespaceId::Type,
            &UnresolvedPath::fake("crate::b::B")
        )
        .is_err());
        assert!(try_to_resolve(
            &db,
            &crate_c,
            &c_other,
            NamespaceId::Type,
            &UnresolvedPath::fake("crate::b_alt::B")
        )
        .is_ok());

        let empty: &[&str] = &[];
        assert_eq!(
            try_to_resolve(
                &db,
                &crate_c,
                &c_other,
                NamespaceId::Type,
                &UnresolvedPath::new(false, empty)
            ),
            Err(ResolveError::Impossible)
        );
        assert_eq!(
            try_to_resolve(
                &db,
                &crate_c,
                &c_other,
                NamespaceId::Type,
                &UnresolvedPath::fake("::nonexistent::crate_")
            ),
            Err(ResolveError::Impossible)
        );
        assert_eq!(
            try_to_resolve(
                &db,
                &crate_c,
                &c_other,
                NamespaceId::Type,
                &UnresolvedPath::fake("::super::super::nonexistent")
            ),
            Err(ResolveError::Impossible)
        );
    }

    #[test]
    fn macro_use() {
        let test_crate_a = (*TEST_CRATE_A).clone();
        let test_crate_b = (*TEST_CRATE_B).clone();

        let db = Db::fake_db();

        let mut crate_a = Crate::new(test_crate_a.clone());
        let a_root = crate_a.add_root_scope(Metadata::fake("{root}")).unwrap();
        crate_a
            .add_binding::<MacroItem>(
                &a_root,
                "test_macro".into(),
                Identity::new(&test_crate_a, &["test_macro"]),
                Visibility::Pub,
                Priority::Explicit,
            )
            .unwrap();
        db.insert_crate(crate_a);

        let mut crate_b = Crate::new(test_crate_b.clone());
        let b_root = crate_b.add_root_scope(Metadata::fake("{root}")).unwrap();
        add_crate_dep(&mut crate_b, &test_crate_a, "crate_a".into()).unwrap();
        add_root_extern_crate(
            &db,
            &mut crate_b,
            &test_crate_a,
            &"crate_a".into(),
            Visibility::InScope(b_root.clone()),
            true,
        )
        .unwrap();

        assert!(crate_b
            .prelude
            .get::<MacroItem>(&"test_macro".into())
            .is_some());
    }

    #[test]
    fn debug() {
        let p = UnresolvedPath::fake("thing::somewhere");
        let p = ResolvingPath {
            path: Cow::from(&p.path),
            rooted: p.rooted,
        };

        assert_eq!(format!("{:?}", p), "thing::somewhere");
    }

    #[test]
    fn init_prelude() {
        let mut with_std = Crate::new((*TEST_CRATE_A).clone());
        add_std_prelude(&mut with_std, false).unwrap();

        assert!(with_std.prelude.get::<TypeItem>(&"Option".into()).is_some());
        assert!(with_std.prelude.get::<Scope>(&"Option".into()).is_some());
        assert!(with_std
            .prelude
            .get::<MacroItem>(&"include_bytes".into())
            .is_some());
        assert!(with_std.prelude.get::<SymbolItem>(&"Some".into()).is_some());

        assert!(with_std.prelude.get::<TypeItem>(&"Vec".into()).is_some());
        assert!(with_std.prelude.get::<Scope>(&"Vec".into()).is_some());

        let mut no_std = Crate::new((*TEST_CRATE_A).clone());
        add_std_prelude(&mut no_std, true).unwrap();

        assert!(no_std.prelude.get::<TypeItem>(&"Option".into()).is_some());
        assert!(no_std.prelude.get::<Scope>(&"Option".into()).is_some());
        assert!(no_std
            .prelude
            .get::<MacroItem>(&"include_bytes".into())
            .is_some());
        assert!(no_std.prelude.get::<SymbolItem>(&"Some".into()).is_some());

        assert!(no_std.prelude.get::<TypeItem>(&"Vec".into()).is_none());
        assert!(no_std.prelude.get::<Scope>(&"Vec".into()).is_none());
    }

    #[test]
    fn find_file() {
        let test_crate_a = (*TEST_CRATE_A).clone();

        let temp_dir = tempdir::TempDir::new("tendon_test").unwrap();
        let dir = temp_dir.path();

        let root = dir.join("root.rs");
        let sub_mod = dir.join("sub_mod");
        let sub_mod_file = sub_mod.join("mod.rs");
        let sub_mod_child = sub_mod.join("child.rs");

        let sub_mod_new = dir.join("sub_mod_new");
        let sub_mod_new_file = dir.join("sub_mod_new.rs");
        let sub_mod_new_child = sub_mod_new.join("child.rs");

        let renamed_file = dir.join("renamed.rs");

        fs::File::create(&root).unwrap();

        fs::create_dir(&sub_mod).unwrap();
        fs::File::create(&sub_mod_file).unwrap();
        fs::File::create(&sub_mod_child).unwrap();

        fs::create_dir(&sub_mod_new).unwrap();
        fs::File::create(&sub_mod_new_file).unwrap();
        fs::File::create(&sub_mod_new_child).unwrap();

        fs::File::create(&renamed_file).unwrap();

        let item_sub_mod = syn::parse_str::<syn::ItemMod>("mod sub_mod;").unwrap();
        let item_sub_mod_new = syn::parse_str::<syn::ItemMod>("mod sub_mod_new;").unwrap();
        let item_child = syn::parse_str::<syn::ItemMod>("mod child;").unwrap();
        let item_renamed =
            syn::parse_str::<syn::ItemMod>("#[path = \"renamed\"] mod thing;").unwrap();

        let mut crate_data = CrateData::fake(test_crate_a.clone());
        crate_data.entry = root.clone();

        let crate_root = Identity::root(&test_crate_a);

        let mut loc = LocationMetadata {
            source_file: root.clone(),
            module_path: crate_root.clone(),
            macro_invocation: None,
            crate_data: &crate_data,
        };

        assert_eq!(
            &find_source_file(&loc, &item_sub_mod).unwrap(),
            &sub_mod_file
        );
        assert_eq!(
            &find_source_file(&loc, &item_sub_mod_new).unwrap(),
            &sub_mod_new_file
        );
        assert_eq!(
            &find_source_file(&loc, &item_renamed).unwrap(),
            &renamed_file
        );

        loc.source_file = sub_mod_file.clone();
        loc.module_path = crate_root.clone_join("sub_mod");
        assert_eq!(
            &find_source_file(&loc, &item_child).unwrap(),
            &sub_mod_child
        );

        loc.source_file = sub_mod_new_file.clone();
        loc.module_path = crate_root.clone_join("sub_mod_new");
        assert_eq!(
            &find_source_file(&loc, &item_child).unwrap(),
            &sub_mod_new_child
        );

        // might wanna add a check for renamed submodules, that can get wacky...
    }
}
