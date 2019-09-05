use tendon_api::attributes::*;
use tendon_api::expressions::*;
use tendon_api::generics::*;
use tendon_api::idents::*;
use tendon_api::items::*;
use tendon_api::paths::Path;
use tendon_api::types::*;

quick_error! {
    #[derive(Clone, Debug)]
    pub enum WalkError {
        Bees
    }
}

/// A type that can be walked to find any unresolved paths it contains.
/// Note: all paths that we'll be resolving will be in the `type` namespace; reexports aren't handled
/// through this trait.
pub trait Resolvable {
    /// Walk the type, passing all unresolved paths to the function F to resolve.
    fn walk<F: FnMut(&mut Path) -> Result<(), WalkError>>(
        &mut self,
        f: &mut F,
    ) -> Result<(), WalkError>;
}

impl Resolvable for Path {
    fn walk<F: FnMut(&mut Path) -> Result<(), WalkError>>(
        &mut self,
        f: &mut F,
    ) -> Result<(), WalkError> {
        match self {
            Path::Unresolved(..) => f(self),
            _ => Ok(()),
        }
    }
}
impl<T: Resolvable> Resolvable for Option<T> {
    fn walk<F: FnMut(&mut Path) -> Result<(), WalkError>>(
        &mut self,
        f: &mut F,
    ) -> Result<(), WalkError> {
        match self {
            Some(t) => t.walk(f),
            _ => Ok(()),
        }
    }
}
impl<T: Resolvable> Resolvable for Vec<T> {
    fn walk<F: FnMut(&mut Path) -> Result<(), WalkError>>(
        &mut self,
        f: &mut F,
    ) -> Result<(), WalkError> {
        for i in self {
            i.walk(f)?;
        }
        Ok(())
    }
}
impl<T: Resolvable, V: Resolvable> Resolvable for (T, V) {
    fn walk<F: FnMut(&mut Path) -> Result<(), WalkError>>(
        &mut self,
        f: &mut F,
    ) -> Result<(), WalkError> {
        self.0.walk(f)?;
        self.1.walk(f)
    }
}

/// Poor person's `#[derive(Resolvable)]`.
/// This macro forces all fields to be explicitly declared, so you can't forget to update it
/// if other places add fields.
/// (Unless the type is marked `skip`, in which case, you still need to go and update it ;)
macro_rules! impl_resolvable {
    (struct $type:ident { $($field:ident),* }) => (
        impl $crate::resolver::resolvable::Resolvable for $type {
            fn walk<F: FnMut(&mut Path) -> Result<(), WalkError>>(&mut self, _f: &mut F) -> Result<(), WalkError> {
                let $type { $($field),* } = self;
                $(
                    $field.walk(_f)?;
                )*
                Ok(())
            }
        }
    );

    (struct $type:ident(_)) => (
        impl $crate::resolver::resolvable::Resolvable for $type {
            fn walk<F: FnMut(&mut Path) -> Result<(), WalkError>>(&mut self, f: &mut F) -> Result<(), WalkError> {
                self.0.walk(f)
            }
        }
    );
    (enum $type:ident { $($variant:ident (_),)* }) => (
        impl $crate::resolver::resolvable::Resolvable for $type {
            fn walk<F: FnMut(&mut Path) -> Result<(), WalkError>>(&mut self, f: &mut F) -> Result<(), WalkError> {
                match self {
                    $(
                        $type::$variant(data) => data.walk(f),
                    )+
                }
            }
        }
    );
    (skip $type:ident) => (
        impl $crate::resolver::resolvable::Resolvable for $type {
            fn walk<F: FnMut(&mut Path) -> Result<(), WalkError>>(&mut self, _: &mut F) -> Result<(), WalkError> {
                Ok(())
            }
        }
    );
}

impl_resolvable!(skip String);
impl_resolvable!(skip bool);
impl_resolvable!(skip Ident);
impl_resolvable!(skip Metadata);
impl_resolvable!(skip SymbolMetadata);
impl_resolvable!(skip Repr);
impl_resolvable!(skip ConstExpr);
impl_resolvable!(skip Expr);
impl_resolvable!(struct TypeMetadata { derives, repr });
impl_resolvable!(skip Lifetime);
impl_resolvable!(struct Trait { path, params, is_maybe });
impl_resolvable!(struct GenericParams { lifetimes, types, type_bindings, consts });
impl_resolvable!(struct TypeBounds { traits, lifetimes });
impl_resolvable!(
    enum Type {
        Path(_),
        Array(_),
        Slice(_),
        Reference(_),
        Pointer(_),
        Tuple(_),
        Never(_),
        QSelf(_),
        BareFn(_),
        ImplTrait(_),
        TraitObject(_),
    }
);
impl_resolvable!(struct PathType { path, params });
impl_resolvable!(struct ArrayType { type_, len });
impl_resolvable!(struct SliceType { type_ });
impl_resolvable!(struct ReferenceType { type_, mut_, lifetime });
impl_resolvable!(struct PointerType { type_, mut_ });
impl_resolvable!(struct TupleType { types });
impl_resolvable!(skip NeverType);
impl_resolvable!(struct QSelfType { self_, trait_, output_ });
impl_resolvable!(struct BareFnType { args, ret, varargs, unsafe_ });
impl_resolvable!(struct ImplTraitType { bounds });
impl_resolvable!(struct TraitObjectType { bounds });
impl_resolvable!(struct ModuleItem { metadata, name });
impl_resolvable!(
    enum SymbolItem {
        Const(_),
        Static(_),
        Function(_),
    }
);
impl_resolvable!(
    enum TypeItem {
        Struct(_),
        Enum(_),
        Trait(_),
    }
);
impl_resolvable!(struct ConstItem { metadata, name, type_, value });
impl_resolvable!(struct StaticItem { metadata, mut_, name, type_, value });
impl_resolvable!(struct ReexportItem { metadata, path });
impl_resolvable!(struct Module { metadata });
impl_resolvable!(struct StructItem { metadata, type_metadata, inherent_impl, generics, name, fields, kind });
impl_resolvable!(struct StructField { metadata, name, type_ });
impl_resolvable!(struct EnumItem { metadata, type_metadata, inherent_impl, generics, name, variants });
impl_resolvable!(struct EnumVariant { metadata, kind, fields, name });
impl_resolvable!(struct UnionItem { metadata, inherent_impl });
impl_resolvable!(struct TraitItem { metadata, inherent_impl });
impl_resolvable!(
    struct InherentImpl {}
);
impl_resolvable!(struct Signature { generics, args, ret, is_unsafe, is_async, is_const, abi, receiver, variadic });
impl_resolvable!(struct FunctionArg { name, type_ });
impl_resolvable!(struct FunctionItem { metadata, symbol_metadata, name, signature });
impl_resolvable!(skip StructKind);
impl_resolvable!(skip Abi);
impl_resolvable!(struct Generics {type_params, lifetime_params, const_params, type_bounds, lifetime_bounds});
impl_resolvable!(skip LifetimeBounds);
impl_resolvable!(struct TypeParameter { name, default });
impl_resolvable!(struct ConstParameter { name, type_, default });

// weird case, don't feel like adding syntax to the macro
impl Resolvable for Receiver {
    fn walk<F: FnMut(&mut Path) -> Result<(), WalkError>>(
        &mut self,
        f: &mut F,
    ) -> Result<(), WalkError> {
        match self {
            Receiver::Other(t) => t.walk(f),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::items::{lower_enum, lower_function_item, lower_struct};
    use crate::walker::WalkModuleCtx;
    use crate::Set;

    #[test]
    fn resolve_all() {
        spoor::init();

        test_ctx!(ctx);

        let struct_ = syn::parse_quote! {
            #[derive(Copy)]
            struct F<T: Trait> {
                f: T,
                other: i32,
                z: <() as Z>::Q,
            }
        };
        let mut struct_ = lower_struct(&ctx, &struct_).unwrap();
        let mut paths = Set::default();
        struct_
            .walk(&mut |p| {
                paths.insert(p.clone());
                Ok(())
            })
            .unwrap();

        assert!(paths.contains(&Path::fake("T")));
        assert!(paths.contains(&Path::fake("i32")));
        assert!(paths.contains(&Path::fake("Copy")));
        assert!(paths.contains(&Path::fake("Z")));
        assert!(paths.contains(&Path::fake("Trait")));

        let enum_ = syn::parse_quote! {
            #[derive(Copy)]
            enum F<'a, T: Trait> {
                A(T),
                B(&'a i32),
                C(Z)
            }
        };
        let mut enum_ = lower_enum(&ctx, &enum_).unwrap();
        let mut paths = Set::default();
        enum_
            .walk(&mut |p| {
                paths.insert(p.clone());
                Ok(())
            })
            .unwrap();

        assert!(paths.contains(&Path::fake("T")));
        assert!(paths.contains(&Path::fake("i32")));
        assert!(paths.contains(&Path::fake("Copy")));
        assert!(paths.contains(&Path::fake("Z")));
        assert!(paths.contains(&Path::fake("Trait")));

        let function_ = syn::parse_quote! {
            fn f<T: Copy, Z: Trait>(self, t: T, v: i32, m: Z) {}
        };
        let mut function_ = lower_function_item(&ctx, &function_).unwrap();
        let mut paths = Set::default();
        function_
            .walk(&mut |p| {
                paths.insert(p.clone());
                Ok(())
            })
            .unwrap();

        assert!(paths.contains(&Path::fake("T")));
        assert!(paths.contains(&Path::fake("i32")));
        assert!(paths.contains(&Path::fake("Copy")));
        assert!(paths.contains(&Path::fake("Z")));
        assert!(paths.contains(&Path::fake("Trait")));
    }
}
