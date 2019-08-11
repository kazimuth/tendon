use super::{LowerError, ModuleCtx};
use crate::lower::{
    attributes::{extract_type_metadata, lower_metadata},
    generics::lower_generics,
    types::lower_type,
};
use syn::spanned::Spanned;
use transgress_api::{
    idents::Ident,
    items::{InherentImpl, StructField, StructItem, StructKind},
};

/// Lower a struct.
pub fn lower_struct(ctx: &ModuleCtx, struct_: &syn::ItemStruct) -> Result<StructItem, LowerError> {
    let mut metadata =
        super::attributes::lower_metadata(ctx, &struct_.vis, &struct_.attrs, struct_.span());
    let type_metadata = extract_type_metadata(&mut metadata)?;

    let generics = lower_generics(&struct_.generics)?;

    let kind = match struct_.fields {
        syn::Fields::Named(..) => StructKind::Named,
        syn::Fields::Unnamed(..) => StructKind::Tuple,
        syn::Fields::Unit => StructKind::Unit,
    };

    let fields = struct_
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let metadata = lower_metadata(ctx, &field.vis, &field.attrs, field.span());
            let name = field
                .ident
                .as_ref()
                .map(|ident| Ident::from(ident))
                .unwrap_or_else(|| Ident::from(&format!("{}", i)[..]));
            let type_ = lower_type(&field.ty)?;

            Ok(StructField {
                metadata,
                name,
                type_,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let inherent_impl = InherentImpl {};

    Ok(StructItem {
        generics,
        metadata,
        fields,
        kind,
        type_metadata,
        inherent_impl,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use transgress_api::attributes::Visibility;
    use transgress_api::paths::Path;
    use transgress_api::types::{PathType, Type};

    #[test]
    fn struct_lowering() {
        let ctx = &ModuleCtx {
            source_file: "fake_file.rs".into(),
        };
        let struct_: syn::ItemStruct = syn::parse_quote! {
            /// This is an example struct.
            #[derive(Clone)]
            pub struct Thing<'a, T> where T: Clone + 'a {
                /// This is a reference to a different thing.
                pub reference: &'a T,
                others: Vec<&'a T>,
                count: i32,
                path: &'a std::path::Path,
            }
        };
        let struct_ = lower_struct(&ctx, &struct_).unwrap();
        println!("{:#?}", struct_);

        assert_eq!(struct_.metadata.visibility, Visibility::Pub);
        assert_eq!(struct_.type_metadata.derives[0].path, Path::fake("Clone"));
        assert_eq!(struct_.kind, StructKind::Named);
        assert_eq!(struct_.fields.len(), 4);
        assert_eq!(struct_.fields[0].name, Ident::from("reference"));
        assert_eq!(struct_.fields[1].name, Ident::from("others"));
        assert_eq!(struct_.fields[2].name, Ident::from("count"));
        assert_eq!(struct_.fields[3].name, Ident::from("path"));

        assert_match!(struct_.fields[2].type_, Type::Path(PathType { path, params }) => {
            assert_eq!(path, &Path::fake("i32"));
            assert!(params.is_empty());
        });

        assert_eq!(struct_.fields[0].metadata.visibility, Visibility::Pub);
        assert_eq!(struct_.fields[1].metadata.visibility, Visibility::NonPub);
        assert_eq!(struct_.fields[2].metadata.visibility, Visibility::NonPub);
        assert_eq!(struct_.fields[3].metadata.visibility, Visibility::NonPub);
        assert_eq!(
            struct_.fields[0].metadata.docs,
            Some(" This is a reference to a different thing.".into())
        );
    }
}
