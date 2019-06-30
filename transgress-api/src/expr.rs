pub use serde::{Deserialize, Serialize};

/// A constant expression.
/// Contained string must be valid Rust code.
#[derive(Clone, Serialize, Deserialize)]
pub struct ConstExpr(pub String);

/// A non-constant expression.
/// Contained string must be valid Rust code.
#[derive(Clone, Serialize, Deserialize)]
pub struct Expr(pub String);
