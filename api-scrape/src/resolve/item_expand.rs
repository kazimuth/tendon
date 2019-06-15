//! Simplified macro expansion for items. Does not handle expressions at all (expands them to ()),
//! since we only need the result for its signature.
//! Implemented as an interpreter on top of syn.

use crate::{Error, Result};
use proc_macro2::TokenStream;
use quote::quote;

pub fn expand(source: &syn::ItemMacro, invocation: &syn::Macro) -> Result<TokenStream> {
    Ok(quote! {})
}
