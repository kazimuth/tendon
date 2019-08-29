use crate::Map;
use syn;
use transgress_api::paths::UnresolvedPath;
use transgress_api::{
    idents::Ident,
    paths::Path,
};

/// Lower a use tree into a set of globs and imports.
pub fn lower_use(use_: &syn::ItemUse, globs: &mut Vec<Path>, imports: &mut Map<Ident, Path>) {
    lower_use_tree(
        &use_.tree,
        globs,
        imports,
        UnresolvedPath {
            is_absolute: use_.leading_colon.is_some(),
            path: vec![],
        },
    )
}

fn lower_use_tree(
    use_: &syn::UseTree,
    globs: &mut Vec<Path>,
    imports: &mut Map<Ident, Path>,
    current: UnresolvedPath,
) {
    match use_ {
        syn::UseTree::Path(path) => lower_use_tree(
            &*path.tree,
            globs,
            imports,
            current.join(Ident::from(&path.ident)),
        ),
        syn::UseTree::Group(group) => {
            for path in group.items.iter() {
                lower_use_tree(path, globs, imports, current.clone());
            }
        }
        syn::UseTree::Glob(_) => globs.push(current.into()),
        syn::UseTree::Name(name) => {
            imports.insert(
                Ident::from(&name.ident),
                current.join(Ident::from(&name.ident)).into(),
            );
        }
        syn::UseTree::Rename(rename) => {
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

    #[test]
    fn lowering() {
        let mut globs = vec![];
        let mut imports = Map::default();

        lower_use(
            &syn::parse_quote! {
                use ::x::y::{z::W, f as p, l::*};
            },
            &mut globs,
            &mut imports,
        );

        assert_eq!(globs[0], Path::fake("::x::y::l"));
        assert_eq!(imports[&Ident::from("W")], Path::fake("::x::y::z::W"));
        assert_eq!(imports[&Ident::from("p")], Path::fake("::x::y::f"));

        globs.clear();
        imports.clear();

        lower_use(
            &syn::parse_quote! {
                pub use x::y::{z::{W, V}, f as p, l::*};
            },
            &mut globs,
            &mut imports,
        );

        assert_eq!(globs[0], Path::fake("x::y::l"));
        assert_eq!(imports[&Ident::from("W")], Path::fake("x::y::z::W"));
        assert_eq!(imports[&Ident::from("V")], Path::fake("x::y::z::V"));
        assert_eq!(imports[&Ident::from("p")], Path::fake("x::y::f"));
    }

}
