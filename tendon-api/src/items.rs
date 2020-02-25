use crate::identities::{Identity, LifetimeId, TypeId};
use crate::tokens::Tokens;
use crate::{
    attributes::{Metadata, SymbolMetadata, TypeMetadata},
    expressions::ConstExpr,
    paths::Ident,
};
use serde::{Deserialize, Serialize};

/// An item in the macro namespace.
#[derive(Serialize, Deserialize)]
pub enum MacroItem {
    Declarative(DeclarativeMacroItem),
    Procedural(ProceduralMacroItem),
    Derive(DeriveMacroItem),
    Attribute(AttributeMacroItem),
}

/// An item in the symbol namespace - a const, static, function, or reexport of the same.
/// (Strictly speaking consts aren't "symbols" as they're not visible to the linker [I think??], but
/// they live in the namespace, so whatever.)
#[derive(Serialize, Deserialize)]
pub enum SymbolItem {
    Const(ConstItem),
    Static(StaticItem),
    Function(FunctionItem),
    ConstParam(ConstParamItem),
}

/// An item in the type namespace.
///
/// Includes generic parameters, which can be lifetimes.
#[derive(Serialize, Deserialize)]
pub enum TypeItem {
    Struct(StructItem),
    Enum(EnumItem),
    Trait(TraitItem),
    TypeParam(TypeParamItem),
    LifetimeParam(LifetimeParamItem),
}

/// A constant `const x: T = expr`, known at compile time,
#[derive(Serialize, Deserialize)]
pub struct ConstItem {
    pub metadata: Metadata,
    pub type_: Box<TypeId>,
    pub value: ConstExpr,
}

/// A static value `static x: T = expr`, stored at a location in memory.
#[derive(Serialize, Deserialize)]
pub struct StaticItem {
    pub metadata: Metadata,
    pub mut_: bool,
    pub type_: Box<TypeId>,
    pub value: String,
}

/// A module.
#[derive(Serialize, Deserialize)]
pub struct Module {
    pub metadata: Metadata,
}

/// A non-tuple struct, `struct Point { x: f32, y: f32 }`
#[derive(Debug, Serialize, Deserialize)]
pub struct StructItem {
    pub metadata: Metadata,
    pub type_metadata: TypeMetadata,
    pub inherent_impl: InherentImpl,
    /// The fields of this struct.
    pub fields: Vec<StructField>,
    /// How this struct is defined.
    pub kind: StructKind,
    /// Maps to a list of
    pub generics: Vec<Identity>,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum StructKind {
    Named,
    Tuple,
    Unit,
}

/// A field of a non-tuple struct.
#[derive(Debug, Serialize, Deserialize)]
pub struct StructField {
    pub metadata: Metadata,
    pub type_: TypeId,
}

/// An enum, `enum Planet { Earth, Mars, Jupiter }`
#[derive(Serialize, Deserialize)]
pub struct EnumItem {
    pub metadata: Metadata,
    pub type_metadata: TypeMetadata,
    pub inherent_impl: InherentImpl,
    pub generic_params: GenericParams,
    /// The variants of this enum.
    pub variants: Vec<EnumVariant>,
}

#[derive(Serialize, Deserialize)]
pub struct EnumVariant {
    pub metadata: Metadata,
    pub kind: StructKind,
    pub fields: Vec<StructField>,
}

/// A union, `union Planet { Earth, Mars, Jupiter }`
#[derive(Serialize, Deserialize)]
pub struct UnionItem {
    pub metadata: Metadata,
    pub inherent_impl: InherentImpl,
}

/// A trait declaration.
#[derive(Serialize, Deserialize)]
pub struct TraitItem {
    pub metadata: Metadata,
    pub inherent_impl: InherentImpl,
}

/// A macro-by-example, `macro_rules!`.
#[derive(Serialize, Deserialize)]
pub struct DeclarativeMacroItem {
    /// Other metadata.
    pub metadata: Metadata,
    /// If this macro is `#[macro_export]`.
    pub macro_export: bool,
    /// Note: currently, macros-by-example are re-parsed every time they're invoked, because the
    /// parsed forms aren't Send / Serialize. This should probably be fixed...
    pub tokens: Tokens,
}

/// A procedural macro (invoked via bang).
#[derive(Serialize, Deserialize)]
pub struct ProceduralMacroItem {
    pub metadata: Metadata,
}

/// A procedural attribute macro.
#[derive(Serialize, Deserialize)]
pub struct AttributeMacroItem {
    pub metadata: Metadata,
}

/// A (procedural) derive macro.
#[derive(Serialize, Deserialize)]
pub struct DeriveMacroItem {
    pub metadata: Metadata,
}

/// The inherent implementation of a type: all methods implemented directly on that type.
#[derive(Debug, Serialize, Deserialize)]
pub struct InherentImpl {}

/// A function (or method).
#[derive(Debug, Serialize, Deserialize)]
pub struct Signature {
    pub generic_params: GenericParams,
    /// The arguments to this function.
    /// Note: this doesn't include `self`, that'll be stored in `Method.receiver` instead
    pub args: Vec<FunctionArg>,
    /// The return type of this function.
    pub ret: TypeId,
    /// If this function is `unsafe`.
    pub is_unsafe: bool,
    /// If this function is `async`.
    pub is_async: bool,
    /// If this function is `const`.
    pub is_const: bool,
    /// What ABI does this use?
    pub abi: Abi,
    /// The receiver. Will always be `Receiver::None` for non-method arguments.
    pub receiver: Receiver,
    /// If this function is variadic.
    pub variadic: bool,
}

/// The abi of a function.
#[derive(Debug, Serialize, Deserialize)]
pub enum Abi {
    Rust,
    C,
    Other(String),
}

/// A standalone function.
#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionItem {
    pub metadata: Metadata,
    pub symbol_metadata: SymbolMetadata,
    /// The signature of this function.
    pub signature: Signature,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionArg {
    pub metadata: Metadata,
    /// The type of the argument.
    pub type_: TypeId,
}

/// The receiver of a method.
#[derive(Debug, Serialize, Deserialize)]
pub enum Receiver {
    /// Doesn't take an instance as an argument.
    /// This is always the case for non-method functions.
    None,
    /// Takes `self`.
    ConsumeSelf,
    /// Takes `&self`.
    RefSelf {
        lifetime: Option<LifetimeId>,
        mut_: bool,
    },
    /// Takes some other form of self (e.g. `self: Pin<&mut Self>`).
    Other(TypeId),
}

/// A type parameter item. These are stored at declaration sites.
#[derive(Serialize, Deserialize, Debug)]
pub struct TypeParamItem {
    pub metadata: Metadata,
    pub type_constraints: Vec<TypeId>,
    pub lifetime_constraints: Vec<LifetimeId>,
    /// The default value of the type parameter, if present.
    pub default: Option<TypeId>,
}

/// A lifetime parameter item. These are stored at declaration sites.
#[derive(Serialize, Deserialize, Debug)]
pub struct LifetimeParamItem {
    pub metadata: Metadata,
    pub constraints: Vec<LifetimeId>,
}

/// A const parameter item. These are stored at declaration sites.
#[derive(Serialize, Deserialize, Debug)]
pub struct ConstParamItem {
    pub metadata: Metadata,
    pub type_: TypeId,
    pub default: Option<ConstExpr>,
}

/// Generics embedded at a use site.
#[derive(Serialize, Deserialize, Debug)]
pub struct GenericParams {
    /// Lifetime parameters to a type.
    pub lifetimes: Vec<LifetimeId>,
    /// Type arguments to a type.
    pub types: Vec<TypeId>,
    /// Type bindings (e.g. `Output=T`)
    pub type_bindings: Vec<(Ident, TypeId)>,
    /// Const generic bindings.
    /// https://github.com/rust-lang/rfcs/blob/master/text/2000-const-generics.md
    /// Note: some of these may be parsed as types unfortunately, need to fix that later
    /// in the pipeline.
    pub consts: Vec<(Ident, ConstExpr)>,
}
impl GenericParams {
    pub fn is_empty(&self) -> bool {
        self.lifetimes.is_empty()
            && self.types.is_empty()
            && self.type_bindings.is_empty()
            && self.consts.is_empty()
    }
}
