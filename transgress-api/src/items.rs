use crate::tokens::Tokens;
use crate::{
    attributes::{Metadata, SymbolMetadata, TypeMetadata},
    expressions::ConstExpr,
    generics::{Generics, Lifetime},
    idents::Ident,
    paths::Path,
    types::Type,
};
use serde::{Deserialize, Serialize};

/// A module.
#[derive(Clone, Serialize, Deserialize)]
pub struct ModuleItem {
    pub metadata: Metadata,
    pub name: Ident,
}

/// An item in the macro namespace.
#[derive(Clone, Serialize, Deserialize)]
pub enum MacroItem {
    Declarative(DeclarativeMacroItem),
    Procedural(ProceduralMacroItem),
    Derive(DeriveMacroItem),
    Attribute(AttributeMacroItem),
}

/// An item in the symbol namespace - a const, static, function, or reexport of the same.
/// (Strictly speaking consts aren't "symbols" as they're not visible to the linker [I think??], but
/// they live in the namespace, so whatever.)
#[derive(Clone, Serialize, Deserialize)]
pub enum SymbolItem {
    Const(ConstItem),
    Static(StaticItem),
    Function(FunctionItem),
}

/// An item in the type namespace.
#[derive(Clone, Serialize, Deserialize)]
pub enum TypeItem {
    Struct(StructItem),
    Enum(EnumItem),
    Trait(TraitItem),
}

/// A constant `const x: T = expr`, known at compile time,
#[derive(Clone, Serialize, Deserialize)]
pub struct ConstItem {
    pub metadata: Metadata,
    pub name: Ident,
    pub type_: Box<Type>,
    pub value: ConstExpr,
}

/// A static value `static x: T = expr`, stored at a location in memory.
#[derive(Clone, Serialize, Deserialize)]
pub struct StaticItem {
    pub metadata: Metadata,
    pub mut_: bool,
    pub name: Ident,
    pub type_: Box<Type>,
    pub value: String,
}

/// A Reexport, `pub use other_location::Thing;`
#[derive(Clone, Serialize, Deserialize)]
pub struct ReexportItem {
    pub metadata: Metadata,
    pub path: Path,
}

/// A module.
#[derive(Clone, Serialize, Deserialize)]
pub struct Module {
    pub metadata: Metadata,
}

/// A non-tuple struct, `struct Point { x: f32, y: f32 }`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StructItem {
    pub metadata: Metadata,
    pub type_metadata: TypeMetadata,
    pub inherent_impl: InherentImpl,
    pub generics: Generics,
    /// The name of the enum.
    pub name: Ident,
    /// The fields of this struct.
    pub fields: Vec<StructField>,
    /// How this struct is defined.
    pub kind: StructKind,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum StructKind {
    Named,
    Tuple,
    Unit,
}

/// A field of a non-tuple struct.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StructField {
    pub metadata: Metadata,
    /// The name of this field. May be a numeral.
    pub name: Ident,
    /// The type of this field.
    pub type_: Type,
}

/// An enum, `enum Planet { Earth, Mars, Jupiter }`
#[derive(Clone, Serialize, Deserialize)]
pub struct EnumItem {
    pub metadata: Metadata,
    pub type_metadata: TypeMetadata,
    pub inherent_impl: InherentImpl,
    pub generics: Generics,
    /// The name of the enum.
    pub name: Ident,
    /// The variants of this enum.
    pub variants: Vec<EnumVariant>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EnumVariant {
    pub metadata: Metadata,
    pub kind: StructKind,
    pub fields: Vec<StructField>,
    pub name: Ident,
}

/// A union, `union Planet { Earth, Mars, Jupiter }`
#[derive(Clone, Serialize, Deserialize)]
pub struct UnionItem {
    pub metadata: Metadata,
    pub inherent_impl: InherentImpl,
}

/// A trait declaration.
#[derive(Clone, Serialize, Deserialize)]
pub struct TraitItem {
    pub metadata: Metadata,
    pub inherent_impl: InherentImpl,
}

/// A macro-by-example, `macro_rules!`.
#[derive(Clone, Serialize, Deserialize)]
pub struct DeclarativeMacroItem {
    pub metadata: Metadata,
    /// The name of the declared macros.
    pub name: Ident,
    /// If this macro is `#[macro_export]`.
    pub macro_export: bool,
    /// Note: currently, macros-by-example are re-parsed every time they're invoked, because the
    /// parsed forms aren't Send / Serialize. This should probably be fixed...
    pub tokens: Tokens,
}

/// A procedural macro (invoked via bang).
#[derive(Clone, Serialize, Deserialize)]
pub struct ProceduralMacroItem {
    pub metadata: Metadata,
}

/// A procedural attribute macro.
#[derive(Clone, Serialize, Deserialize)]
pub struct AttributeMacroItem {
    pub metadata: Metadata,
}

/// A (procedural) derive macro.
#[derive(Clone, Serialize, Deserialize)]
pub struct DeriveMacroItem {
    pub metadata: Metadata,
}

/// The inherent implementation of a type: all methods implemented directly on that type.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InherentImpl {}

/// A function (or method).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Signature {
    pub generics: Generics,
    /// The arguments to this function.
    /// Note: this doesn't include `self`, that'll be stored in `Method.receiver` instead
    pub args: Vec<FunctionArg>,
    /// The return type of this function.
    pub ret: Type,
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Abi {
    Rust,
    C,
    Other(String),
}

/// A standalone function.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionItem {
    pub metadata: Metadata,
    pub symbol_metadata: SymbolMetadata,
    /// The name of this function.
    pub name: Ident,
    /// The signature of this function.
    pub signature: Signature,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionArg {
    /// The name of the argument.
    pub name: Ident,
    /// The type of the argument.
    pub type_: Type,
}

/// The receiver of a method.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Receiver {
    /// Doesn't take an instance as an argument.
    /// This is always the case for non-method functions.
    None,
    /// Takes `self`.
    ConsumeSelf,
    /// Takes `&self`.
    RefSelf {
        lifetime: Option<Lifetime>,
        mut_: bool,
    },
    /// Takes some other form of self (e.g. `self: Pin<&mut Self>`).
    Other(Type),
}
