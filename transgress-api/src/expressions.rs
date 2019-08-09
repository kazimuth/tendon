//! Expressions. This module is fairly emaciated since we mostly don't handle these.

use crate::tokens::Tokens;
pub use serde::{Deserialize, Serialize};

/// A constant expression.
/// Represented as uninterpreted tokens.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConstExpr(pub Tokens);

/// A non-constant expression.
/// Represented as uninterpreted tokens.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Expr(pub Tokens);
