use crate::{
    attributes::{Metadata, TypeMetadata},
    expressions::ConstExpr,
    generics::Generics,
    idents::Ident,
    paths::{GenericPath, Path},
    types::Type,
};
use serde::{Deserialize, Serialize};

/// A module.
pub struct ModuleItem {
    pub metadata: Metadata,
}

/// An item in the macro namespace.
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
    pub name: Ident,
    pub type_: Box<Type>,
    pub value: ConstExpr,
}

/// A static value `static x: T = expr`, stored at a location in memory.
#[derive(Clone, Serialize, Deserialize)]
pub struct StaticItem {
    pub mut_: bool,
    pub name: Ident,
    pub type_: Box<Type>,
    pub value: String,
}

/// A standalone function, `fn f(x: i32) -> i32 { ... }`
#[derive(Clone, Serialize, Deserialize)]
pub struct FunctionItem {}

/// A Reexport, `pub use other_location::Thing;`
#[derive(Clone, Serialize, Deserialize)]
pub struct ReexportItem {
    pub path: Path,
}

/// A module.
#[derive(Clone, Serialize, Deserialize)]
pub struct Module {
    pub metadata: Metadata,
}

/// A non-tuple struct, `struct Point { x: f32, y: f32 }`
#[derive(Clone, Serialize, Deserialize)]
pub struct StructItem {
    pub metadata: Metadata,
    pub type_metadata: TypeMetadata,
    pub inherent_impl: InherentImpl,
    /// The fields of this struct.
    pub fields: Vec<StructField>,
    /// How this struct is defined.
    pub kind: StructKind,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum StructKind {
    Named,
    Tuple,
    Unit,
}

/// A field of a non-tuple struct.
#[derive(Clone, Serialize, Deserialize)]
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
    pub inherent_impl: InherentImpl,
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

/// A declarative macro, `macro_rules!`.
#[derive(Clone, Serialize, Deserialize)]
pub struct DeclarativeMacroItem {
    pub metadata: Metadata,
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
/// TODO: how to handle references &ct?
#[derive(Clone, Serialize, Deserialize)]
pub struct InherentImpl {}
