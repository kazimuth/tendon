use serde::{Deserialize, Serialize};

mod ident;

pub use ident::Ident;

#[derive(Clone, Serialize, Deserialize)]
pub struct Crate {
    pub root_module: Module,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Module {
    pub metadata: Metadata,
    pub submodules: Vec<Module>,
    pub enums: Vec<Enum>,
    pub structs: Vec<Struct>,
    pub free_functions: Vec<Function>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub docs: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Struct {
    pub metadata: Metadata,
    pub inherent_impl: InherentImpl,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Enum {
    pub metadata: Metadata,
    pub inherent_impl: InherentImpl,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Function {
    pub ident: Ident,
    pub full_path: Path,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct InherentImpl {
    pub methods: Vec<Function>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Path {}

#[derive(Clone, Serialize, Deserialize)]
pub struct TypeData {
    pub copy: bool,
    pub clone: bool,
    pub send: bool,
    pub sync: bool,
    pub sized: bool,
    pub unpin: bool,
    pub repr_c: bool,
}

#[cfg(test)]
mod tests {}
