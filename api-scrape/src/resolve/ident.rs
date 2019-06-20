use std::ops::Deref;

/// An identifier.
/// TODO: intern
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ident(Box<str>);

impl From<&str> for Ident {
    fn from(s: &str) -> Self {
        Ident(s.to_string().into())
    }
}
impl From<String> for Ident {
    fn from(s: String) -> Self {
        Ident(s.into())
    }
}
impl From<&proc_macro2::Ident> for Ident {
    fn from(s: &proc_macro2::Ident) -> Ident {
        Ident(s.to_string().into())
    }
}

impl Deref for Ident {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}
impl std::borrow::Borrow<str> for Ident {
    fn borrow(&self) -> &str {
        &*self
    }
}
