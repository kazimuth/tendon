//! Built-in names, like `i32`, `char`, `'static`.
//!
//! These names will never actually appear in a database but are considered to exist anyway.

use crate::identities::{CrateId, Identity, TypeId, PathType, LifetimeId};

lazy_static! {
    /// The fake crate we store builtins in.
    pub static ref BUILTINS_CRATE: CrateId = CrateId::new("{builtin}", "0.0.0");
    pub static ref CORE_CRATE: CrateId = CrateId::new("core", "0.0.0");
    pub static ref ALLOC_CRATE: CrateId = CrateId::new("alloc", "0.0.0");
    pub static ref TEST_CRATE: CrateId = CrateId::new("test", "0.0.0");
    pub static ref PROC_MACRO_CRATE: CrateId = CrateId::new("test", "0.0.0");
    pub static ref STD_CRATE: CrateId = CrateId::new("test", "0.0.0");

    pub static ref STR: TypeId = TypeId::Path(PathType { path: Identity::new(&*BUILTINS_CRATE, &["str"]), params: Default::default() });
    pub static ref CHAR: TypeId = TypeId::Path(PathType { path: Identity::new(&*BUILTINS_CRATE, &["char"]), params: Default::default() });
    pub static ref I8: TypeId = TypeId::Path(PathType { path: Identity::new(&*BUILTINS_CRATE, &["i8"]), params: Default::default() });
    pub static ref I16: TypeId = TypeId::Path(PathType { path: Identity::new(&*BUILTINS_CRATE, &["i16"]), params: Default::default() });
    pub static ref I32: TypeId = TypeId::Path(PathType { path: Identity::new(&*BUILTINS_CRATE, &["i32"]), params: Default::default() });
    pub static ref I64: TypeId = TypeId::Path(PathType { path: Identity::new(&*BUILTINS_CRATE, &["i64"]), params: Default::default() });
    pub static ref I128: TypeId = TypeId::Path(PathType { path: Identity::new(&*BUILTINS_CRATE, &["i128"]), params: Default::default() });
    pub static ref ISIZE: TypeId = TypeId::Path(PathType { path: Identity::new(&*BUILTINS_CRATE, &["isize"]), params: Default::default() });
    pub static ref U8: TypeId = TypeId::Path(PathType { path: Identity::new(&*BUILTINS_CRATE, &["u8"]), params: Default::default() });
    pub static ref U16: TypeId = TypeId::Path(PathType { path: Identity::new(&*BUILTINS_CRATE, &["u16"]), params: Default::default() });
    pub static ref U32: TypeId = TypeId::Path(PathType { path: Identity::new(&*BUILTINS_CRATE, &["u32"]), params: Default::default() });
    pub static ref U64: TypeId = TypeId::Path(PathType { path: Identity::new(&*BUILTINS_CRATE, &["u64"]), params: Default::default() });
    pub static ref U128: TypeId = TypeId::Path(PathType { path: Identity::new(&*BUILTINS_CRATE, &["u128"]), params: Default::default() });
    pub static ref USIZE: TypeId = TypeId::Path(PathType { path: Identity::new(&*BUILTINS_CRATE, &["USIZE"]), params: Default::default() });

    pub static ref STATIC: LifetimeId = LifetimeId::new(Identity::new(&*BUILTINS_CRATE, &["'static"]));

    //    str
    //    i8
    //    i16
    //    i32
    //    i64
    //    u8
    //    u16
    //    u32
    //    u64
    //    isize
    //    usize
    //    f32
    //    f64
}
