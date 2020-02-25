use crate::lower::attributes::lower_visibility;
use crate::walker::ModuleScope;
use tendon_api::attributes::Visibility;
use tendon_api::paths::Ident;
use tendon_api::paths::UnresolvedPath;

/// Lower a use tree into a set of globs and imports.
pub(crate) fn lower_use(scope: &mut ModuleScope, use_: &syn::ItemUse) {
    // TODO: do we need to care about metadata here?

    let vis = lower_visibility(&use_.vis);
    lower_use_tree(
        scope,
        &use_.tree,
        &vis,
        UnresolvedPath {
            rooted: use_.leading_colon.is_some(),
            path: vec![],
        },
    );
}

fn lower_use_tree(
    scope: &mut ModuleScope,
    use_: &syn::UseTree,
    vis: &Visibility,
    current: UnresolvedPath,
) {
    match use_ {
        syn::UseTree::Path(path) => lower_use_tree(
            scope,
            &*path.tree,
            vis,
            current.join(Ident::from(&path.ident)),
        ),
        syn::UseTree::Group(group) => {
            for path in group.items.iter() {
                lower_use_tree(scope, path, vis, current.clone());
            }
        }
        syn::UseTree::Glob(_) => {
            let globs = match vis {
                Visibility::Pub => &mut scope.pub_glob_imports,
                Visibility::NonPub => &mut scope.glob_imports,
            };
            globs.push(current.into())
        }
        syn::UseTree::Name(name) => {
            let imports = match vis {
                Visibility::Pub => &mut scope.pub_imports,
                Visibility::NonPub => &mut scope.imports,
            };

            imports.insert(
                Ident::from(&name.ident),
                current.join(Ident::from(&name.ident)).into(),
            );
        }
        syn::UseTree::Rename(rename) => {
            let imports = match vis {
                Visibility::Pub => &mut scope.pub_imports,
                Visibility::NonPub => &mut scope.imports,
            };

            imports.insert(
                Ident::from(&rename.rename),
                current.join(Ident::from(&rename.ident)).into(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tendon_api::paths::Path;

    #[test]
    fn lowering() {
        let mut scope = ModuleScope::default();

        lower_use(
            &mut scope,
            &syn::parse_quote! {
                use ::x::y::{z::W, f as p, l::*};
            },
        );

        assert_eq!(scope.glob_imports[0], Path::fake("::x::y::l"));
        assert_eq!(scope.imports[&Ident::from("W")], Path::fake("::x::y::z::W"));
        assert_eq!(scope.imports[&Ident::from("p")], Path::fake("::x::y::f"));

        let mut scope = ModuleScope::default();
        lower_use(
            &mut scope,
            &syn::parse_quote! {
                pub use x::y::{z::{W, V}, f as p, l::*};
            },
        );

        assert_eq!(scope.pub_glob_imports[0], Path::fake("x::y::l"));
        assert_eq!(
            scope.pub_imports[&Ident::from("W")],
            Path::fake("x::y::z::W")
        );
        assert_eq!(
            scope.pub_imports[&Ident::from("V")],
            Path::fake("x::y::z::V")
        );
        assert_eq!(scope.pub_imports[&Ident::from("p")], Path::fake("x::y::f"));
    }
}
