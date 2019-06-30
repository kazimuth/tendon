use crate::{expr::ConstExpr, Ident, Path, Trait, Type};
use serde::{Deserialize, Serialize};

pub mod helpers;

pub use helpers::{InherentImpl, ItemMetadata};

/// An item in the symbol namespace - a const, static, function, or reexport of the same.
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
    name: Ident,
    type_: Box<Type>,
    value: ConstExpr,
}

/// A static value `static x: T = expr`, stored at a location in memory.
#[derive(Clone, Serialize, Deserialize)]
pub struct StaticItem {
    mut_: bool,
    name: Ident,
    type_: Box<Type>,
    value: String,
}

/// A standalone function, `fn f(x: i32) -> i32 { ... }`
#[derive(Clone, Serialize, Deserialize)]
pub struct FunctionItem {
    pub ident: Ident,
    pub full_path: Path,
}
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

#[derive(Clone, Serialize, Deserialize)]
pub struct ModuleItem {
    pub item_metadata: ItemMetadata,
}
