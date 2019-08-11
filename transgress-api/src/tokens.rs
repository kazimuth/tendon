//! Uninterpreted tokens for passing around.
use quote::ToTokens;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A series of rust tokens, stored as a string.
/// You might ask, why not just use proc_macro2's `TokenStream`?
/// Well, that isn't Serialize or Send, this type is.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tokens(String);
impl<T: ToTokens> From<T> for Tokens {
    fn from(t: T) -> Self {
        Tokens(t.into_token_stream().to_string())
    }
}
impl Tokens {
    /// Create tokens from a string. The string must be a sequence of valid rust tokens, with balanced
    /// delimiters.
    pub fn new(tokens: &str) -> Result<Self, syn::Error> {
        syn::parse_str::<proc_macro2::TokenStream>(tokens)?;
        Ok(Tokens(tokens.to_string()))
    }

    /// Get the tokens back out of this type.
    pub fn get_tokens(&self) -> proc_macro2::TokenStream {
        syn::parse_str::<proc_macro2::TokenStream>(&self.0)
            .expect("invariant violated: Tokens can only contain valid rust tokens")
    }

    /// Parse as something.
    pub fn parse<P: syn::parse::Parse>(&self) -> Result<P, syn::Error> {
        syn::parse_str(&self.0)
    }
}
impl fmt::Debug for Tokens {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl ToString for Tokens {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}
