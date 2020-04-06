use super::{LocationMetadata, WalkError};
use crate::lower::attributes::extract_attribute;
use crate::walker::Walker;
use std::borrow::Cow;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use syn;
use tendon_api::attributes::{Metadata, Span, Visibility};
use tendon_api::builtins::{ALLOC_CRATE, BUILTIN_TYPES, CORE_CRATE, STD_CRATE};
use tendon_api::crates::CrateData;
use tendon_api::database::{Crate, Db, NamespaceLookup};
use tendon_api::identities::{CrateId, Identity};
use tendon_api::items::{MacroItem, SymbolItem, TypeItem};
use tendon_api::paths::{Ident, UnresolvedPath};
use tendon_api::scopes::{Binding, NamespaceId, Priority, Scope};
use tendon_api::Map;
use tracing::{error, info};

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

    try_to_resolve_rec(db, crate_in_progress, in_module, in_module, namespace, path)
}

fn try_to_resolve_rec(
    db: &Db,
    crate_in_progress: &Crate,
    orig_module: &Identity,
    in_module: &Identity,
    namespace_id: NamespaceId,
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
            path,
        );
    }

    let get_binding_by = |namespace_id, ident| -> Result<Identity, ResolveError> {
        let binding = in_crate
            .get_binding_by(in_module, namespace_id, ident)
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
pub fn add_crate_dep(crate_: &mut Crate, name: Ident, extern_crate_id: &CrateId) {
    if let Some(_) = crate_
        .extern_crate_bindings
        .insert(name.clone(), extern_crate_id.clone())
    {
        panic!("can't add crate dep twice!");
    }
    crate_.add_prelude_binding_by(NamespaceId::Scope, name, Identity::root(extern_crate_id));
}

/// Add an `extern crate` statement. Crate should have already been added
/// and root scope should exist. Handles importing `#[macro_use]` macros, which are added to the *prelude* (not the textual scopes!).
/// See also `add_crate_dep`.
///
/// Subtle effects:
/// ```no_build
/// pub extern crate core as core_;
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
    rename: Option<&Ident>,
    visibility: Visibility,
    macro_use: bool,
) -> Result<(), WalkError> {
    let extern_crate_root_id = Identity::root(extern_crate_id);

    // find the name to add to the root scope
    let name = if let Some(rename) = rename {
        // add rename to scopes too
        add_crate_dep(crate_, rename.clone(), extern_crate_id);
        rename.clone()
    } else {
        let name = Ident::from(&extern_crate_id.name[..]);
        name
    };
    // add dep as `crate::dep`
    crate_.add_binding::<Scope>(
        &Identity::root(&crate_.id),
        name,
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
        let identity = Identity::new(crate_id, &path_);

        crate_.add_prelude_binding_by(I::namespace_id(), last, identity)?;

        Ok(())
    }

    let core_ = &*CORE_CRATE;
    let alloc_ = &*ALLOC_CRATE;
    let std_ = &*STD_CRATE;

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
    use crate::walker::UnexpandedItem::UnresolvedMacroInvocation;
    use tendon_api::attributes::TypeMetadata;
    use tendon_api::database::Db;
    use tendon_api::identities::{TEST_CRATE_A, TEST_CRATE_B};
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

        let in_root = crate_
            .get_binding::<TypeItem>(&root, &"CRenamed".into())
            .unwrap();
        let in_a_b = crate_
            .get_binding::<TypeItem>(&mod_a_b, &"C".into())
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

        /*
        /// Add a scope for attached functions
        let scope_a_b_c = crate.add(&mod_a_b, Scope::new(Metadata::fake("C"), false)).unwrap();
        /// And the invisible {impl} scope; which inherits from the containing module.
        ///
        let scope_a_b_c_impl = crate_.add(&scope_a_b_c, Scope::new(Metadata::fake("{impl}"), false, Some(mod_a_b.clone()))).unwrap();
        */
    }

    #[test]
    fn resolve_deps() {
        let test_crate_a = (*TEST_CRATE_A).clone();
        let mut crate_ = Crate::new(test_crate_a.clone());
        let a_root = crate_.add_root_scope(Metadata::fake("{root}")).unwrap();
        let a_b = crate_
            .add(&a_root, Scope::new(Metadata::fake("b"), true))
            .unwrap();
    }
}
