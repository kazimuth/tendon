use crate::{Result,Error};

//! Simplified macro expansion for items. Does not handle expressions at all (expands them to ()),
//! since we only need the result for its signature.
//! Implemented as an interpreter on top of syn.

pub fn expand(source: &syn::ItemMacro, invocation: &syn::Macro) -> Result<syn::TokenStream> {
    unimplemented!()
}