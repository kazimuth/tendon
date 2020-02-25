//! Expressions. This module is fairly emaciated since we mostly don't handle these.

use crate::tokens::Tokens;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A constant expression.
/// Represented as uninterpreted tokens.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConstExpr(pub Tokens);
impl fmt::Debug for ConstExpr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

/// A non-constant expression.
/// Represented as uninterpreted tokens.
#[derive(Clone, Serialize, Deserialize)]
pub struct Expr(pub Tokens);
impl fmt::Debug for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}
