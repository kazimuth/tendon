//! Built-in names, like `i32`, `char`, `'static`.
//!
//! These names will never actually appear in a database but are considered to exist anyway.

use crate::database::NamespaceLookup;
use crate::identities::{CrateId, Identity};
use crate::paths::Ident;
use crate::scopes::NamespaceId;
use crate::Map;

lazy_static! {
    /// The fake crate we store builtins in.
    pub static ref BUILTINS_CRATE: CrateId = CrateId::new("{builtin}", "0.0.0");
    pub static ref CORE_CRATE: CrateId = CrateId::new("core", "0.0.0");
    pub static ref ALLOC_CRATE: CrateId = CrateId::new("alloc", "0.0.0");
    pub static ref TEST_CRATE: CrateId = CrateId::new("test", "0.0.0");
    pub static ref PROC_MACRO_CRATE: CrateId = CrateId::new("test", "0.0.0");
    pub static ref STD_CRATE: CrateId = CrateId::new("test", "0.0.0");

    pub static ref STR: Identity =  Identity::new(&*BUILTINS_CRATE, &["str"]);
    pub static ref CHAR: Identity =  Identity::new(&*BUILTINS_CRATE, &["char"]);
    pub static ref I8: Identity =  Identity::new(&*BUILTINS_CRATE, &["i8"]);
    pub static ref I16: Identity =  Identity::new(&*BUILTINS_CRATE, &["i16"]);
    pub static ref I32: Identity =  Identity::new(&*BUILTINS_CRATE, &["i32"]);
    pub static ref I64: Identity =  Identity::new(&*BUILTINS_CRATE, &["i64"]);
    pub static ref I128: Identity =  Identity::new(&*BUILTINS_CRATE, &["i128"]);
    pub static ref ISIZE: Identity =  Identity::new(&*BUILTINS_CRATE, &["isize"]);
    pub static ref U8: Identity =  Identity::new(&*BUILTINS_CRATE, &["u8"]);
    pub static ref U16: Identity =  Identity::new(&*BUILTINS_CRATE, &["u16"]);
    pub static ref U32: Identity =  Identity::new(&*BUILTINS_CRATE, &["u32"]);
    pub static ref U64: Identity =  Identity::new(&*BUILTINS_CRATE, &["u64"]);
    pub static ref U128: Identity =  Identity::new(&*BUILTINS_CRATE, &["u128"]);
    pub static ref USIZE: Identity =  Identity::new(&*BUILTINS_CRATE, &["usize"]);
    pub static ref STATIC: Identity = Identity::new(&*BUILTINS_CRATE, &["'static"]);

    pub static ref BUILTIN_TYPES: Map<Ident, Identity> = {
        let mut result = Map::default();
        result.insert("char".into(), CHAR.clone());
        result.insert("i8".into(), I8.clone());
        result.insert("i16".into(), I16.clone());
        result.insert("i32".into(), I32.clone());
        result.insert("i64".into(), I64.clone());
        result.insert("i128".into(), I128.clone());
        result.insert("isize".into(), ISIZE.clone());
        result.insert("u8".into(), U8.clone());
        result.insert("u16".into(), U16.clone());
        result.insert("u32".into(), U32.clone());
        result.insert("u64".into(), U64.clone());
        result.insert("u128".into(), U128.clone());
        result.insert("usize".into(), USIZE.clone());
        result.insert("'static".into(), STATIC.clone());
        result
    };
}

/// Get the builtins table for a namespace, if one exists.
pub fn get_builtins<I: NamespaceLookup>() -> Option<&'static Map<Ident, Identity>> {
    if I::namespace_id() == NamespaceId::Type {
        Some(&*BUILTIN_TYPES)
    } else {
        None
    }
}
