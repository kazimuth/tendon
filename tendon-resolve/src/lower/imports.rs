use crate::lower::attributes::lower_visibility;
use crate::walker::WalkModuleCtx;
use tendon_api::attributes::Visibility;
use tendon_api::idents::Ident;
use tendon_api::paths::UnresolvedPath;


/// Lower a use tree into a set of globs and imports.
pub fn lower_use(ctx: &mut WalkModuleCtx, use_: &syn::ItemUse) {
    // TODO: use this?
    // let metadata = lower_metadata(ctx, &use_.vis, &use_.attrs, use_.span());
    let vis = lower_visibility(&use_.vis);
    lower_use_tree(
        ctx,
        &use_.tree,
        &vis,
        UnresolvedPath {
            is_absolute: use_.leading_colon.is_some(),
            path: vec![],
        },
    );
}

fn lower_use_tree(
    ctx: &mut WalkModuleCtx,
    use_: &syn::UseTree,
    vis: &Visibility,
    current: UnresolvedPath,
) {
    match use_ {
        syn::UseTree::Path(path) => lower_use_tree(
            ctx,
            &*path.tree,
            vis,
            current.join(Ident::from(&path.ident)),
        ),
        syn::UseTree::Group(group) => {
            for path in group.items.iter() {
                lower_use_tree(ctx, path, vis, current.clone());
            }
        }
        syn::UseTree::Glob(_) => {
            let globs = match vis {
                Visibility::Pub => &mut ctx.scope.pub_glob_imports,
                Visibility::NonPub => &mut ctx.scope.glob_imports,
            };
            globs.push(current.into())
        }
        syn::UseTree::Name(name) => {
            let imports = match vis {
                Visibility::Pub => &mut ctx.scope.pub_imports,
                Visibility::NonPub => &mut ctx.scope.imports,
            };

            imports.insert(
                Ident::from(&name.ident),
                current.join(Ident::from(&name.ident)).into(),
            );
        }
        syn::UseTree::Rename(rename) => {
            let imports = match vis {
                Visibility::Pub => &mut ctx.scope.pub_imports,
                Visibility::NonPub => &mut ctx.scope.imports,
            };

            imports.insert(
                Ident::from(&rename.rename),
                current.join(Ident::from(&rename.ident)).into(),
            );
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use tendon_api::paths::Path;

    #[test]
    fn lowering() {
        test_ctx!(mut ctx);

        lower_use(
            &mut ctx,
            &syn::parse_quote! {
                use ::x::y::{z::W, f as p, l::*};
            },
        );

        assert_eq!(ctx.scope.glob_imports[0], Path::fake("::x::y::l"));
        assert_eq!(
            ctx.scope.imports[&Ident::from("W")],
            Path::fake("::x::y::z::W")
        );
        assert_eq!(
            ctx.scope.imports[&Ident::from("p")],
            Path::fake("::x::y::f")
        );

        test_ctx!(mut ctx);
        lower_use(
            &mut ctx,
            &syn::parse_quote! {
                pub use x::y::{z::{W, V}, f as p, l::*};
            },
        );

        assert_eq!(ctx.scope.pub_glob_imports[0], Path::fake("x::y::l"));
        assert_eq!(
            ctx.scope.pub_imports[&Ident::from("W")],
            Path::fake("x::y::z::W")
        );
        assert_eq!(
            ctx.scope.pub_imports[&Ident::from("V")],
            Path::fake("x::y::z::V")
        );
        assert_eq!(
            ctx.scope.pub_imports[&Ident::from("p")],
            Path::fake("x::y::f")
        );
    }
}
