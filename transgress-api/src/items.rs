use crate::{expr::ConstExpr, Ident, Path, Type, attributes::{ItemMetadata}};
use serde::{Deserialize, Serialize};

/// A module.
pub struct ModuleItem {
    pub item_metadata: ItemMetadata,
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
    TupleStruct(TupleStructItem),
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
    pub item_metadata: ItemMetadata,
}

/// A non-tuple struct, `struct Point { x: f32, y: f32 }`
#[derive(Clone, Serialize, Deserialize)]
pub struct StructItem {
    pub item_metadata: ItemMetadata,
    pub inherent_impl: InherentImpl,
}

/// A tuple struct, `struct Point(f32, f32);`
#[derive(Clone, Serialize, Deserialize)]
pub struct TupleStructItem {
    pub item_metadata: ItemMetadata,
    pub inherent_impl: InherentImpl,
}

/// An enum, `enum Planet { Earth, Mars, Jupiter }`
#[derive(Clone, Serialize, Deserialize)]
pub struct EnumItem {
    pub item_metadata: ItemMetadata,
    pub inherent_impl: InherentImpl,
}

/// A union, `union Planet { Earth, Mars, Jupiter }`
#[derive(Clone, Serialize, Deserialize)]
pub struct UnionItem {
    pub item_metadata: ItemMetadata,
    pub inherent_impl: InherentImpl,
}

/// A trait declaration.
#[derive(Clone, Serialize, Deserialize)]
pub struct TraitItem {
    pub item_metadata: ItemMetadata,
    pub inherent_impl: InherentImpl,
}

/// A declarative macro, `macro_rules!`.
#[derive(Clone, Serialize, Deserialize)]
pub struct DeclarativeMacroItem {
    pub item_metadata: ItemMetadata,
}

/// A procedural macro (invoked via bang).
#[derive(Clone, Serialize, Deserialize)]
pub struct ProceduralMacroItem {
    pub item_metadata: ItemMetadata,
}
/// A procedural attribute macro.
#[derive(Clone, Serialize, Deserialize)]
pub struct AttributeMacroItem {
    pub item_metadata: ItemMetadata,
}

/// A (procedural) derive macro.
#[derive(Clone, Serialize, Deserialize)]
pub struct DeriveMacroItem {
    pub item_metadata: ItemMetadata,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct InherentImpl {}
