use super::{LowerError, ModuleCtx};
use crate::lower::{
    attributes::{extract_type_metadata, lower_metadata},
    generics::lower_generics,
    types::lower_type,
};
use syn::spanned::Spanned;
use transgress_api::{
    idents::Ident,
    items::{InherentImpl, StructField, StructItem, StructKind, EnumItem, EnumVariant},
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

    let fields = lower_fields(ctx, &struct_.fields)?;

    let inherent_impl = InherentImpl {};

    let name = Ident::from(&struct_.ident);

    Ok(StructItem {
        name,
        generics,
        metadata,
        fields,
        kind,
        type_metadata,
        inherent_impl,
    })
}

/// Lower an enum.
pub fn lower_enum(ctx: &ModuleCtx, enum_: &syn::ItemEnum) -> Result<EnumItem, LowerError> {
    let mut metadata =
        super::attributes::lower_metadata(ctx, &enum_.vis, &enum_.attrs, enum_.span());
    let type_metadata = extract_type_metadata(&mut metadata)?;


    let generics = lower_generics(&enum_.generics)?;

    let variants = enum_.variants.iter().map(|variant| {
        // Note: we copy the parent's visibility:
        let metadata = super::attributes::lower_metadata(ctx, &enum_.vis, &variant.attrs, variant.span());

        let kind = match variant.fields {
            syn::Fields::Named(..) => StructKind::Named,
            syn::Fields::Unnamed(..) => StructKind::Tuple,
            syn::Fields::Unit => StructKind::Unit,
        };

        let fields = lower_fields(ctx, &variant.fields)?;

        let name = Ident::from(&variant.ident);
        Ok(EnumVariant { metadata, kind, fields, name })
    }).collect::<Result<Vec<_>, _>>()?;

    let inherent_impl = InherentImpl {};

    let name = Ident::from(&enum_.ident);

    Ok(EnumItem {
        name,
        generics,
        metadata,
        type_metadata,
        inherent_impl,
        variants
    })
}

fn lower_fields(ctx: &ModuleCtx, fields: &syn::Fields) -> Result<Vec<StructField>, LowerError> {
    fields
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
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use transgress_api::paths::Path;
    use transgress_api::types::{PathType, Type};
    use transgress_api::attributes::Visibility;

    #[test]
    fn struct_lowering() {
        let ctx = ModuleCtx {
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

        assert_eq!(struct_.name, Ident::from("Thing"));

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

    #[test]
    fn enum_lowering() {
        let ctx = ModuleCtx {
            source_file: "fake_file.rs".into(),
        };
        let enum_: syn::ItemEnum = syn::parse_quote! {
            pub enum Thing2 {
                /// enum variant
                #[attribute = "banana"]
                Variant1,
                Variant2(i32),
                Variant3 { val: i32 }
            }
        };
        let enum_ = lower_enum(&ctx, &enum_).unwrap();

        assert_eq!(enum_.name, Ident::from("Thing2"));
        assert_eq!(enum_.variants.len(), 3);
        assert_eq!(enum_.variants[0].name, Ident::from("Variant1"));
        assert_eq!(enum_.variants[1].name, Ident::from("Variant2"));
        assert_eq!(enum_.variants[2].name, Ident::from("Variant3"));
        assert_eq!(enum_.variants[0].kind, StructKind::Unit);
        assert_eq!(enum_.variants[1].kind, StructKind::Tuple);
        assert_eq!(enum_.variants[2].kind, StructKind::Named);

        assert_eq!(enum_.variants[0].metadata.docs, Some(" enum variant".into()));
        assert_eq!(enum_.variants[0].metadata.extra_attributes.len(), 1);
        assert_eq!(enum_.variants[0].metadata.extra_attributes[0].path(), &Path::fake("attribute"));
        assert_eq!(enum_.variants[0].metadata.extra_attributes[0].get_assigned_string(), Some("banana".into()));

        assert_eq!(enum_.variants[0].fields.len(), 0);
        assert_eq!(enum_.variants[1].fields.len(), 1);
        assert_eq!(enum_.variants[2].fields.len(), 1);

        assert_eq!(enum_.variants[1].fields[0].name, Ident::from("0"));
        assert_eq!(enum_.variants[2].fields[0].name, Ident::from("val"));

    }
}
