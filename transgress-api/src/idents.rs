//! Fast `Ident` type currently implemented using the `SmolStr` crate.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use smol_str::SmolStr;
use std::fmt;
use std::ops::Deref;

/// A rust identifier.
/// Represented using a small-string optimization.
/// TODO: make sure raw idents work.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Ident(SmolStr);

impl Serialize for Ident {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s: &str = &self.0;
        s.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Ident {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Ident(<&str>::deserialize(deserializer)?.into()))
    }
}

impl From<&str> for Ident {
    fn from(s: &str) -> Self {
        Ident(s.into())
    }
}
impl From<String> for Ident {
    fn from(s: String) -> Self {
        Ident(s.into())
    }
}
impl From<&proc_macro2::Ident> for Ident {
    fn from(s: &proc_macro2::Ident) -> Ident {
        // TODO: could optimize this w/ a thread-local string buffer
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

impl std::fmt::Debug for Ident {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ident_serialize() {
        assert_eq!(
            serde_json::from_str::<Ident>(&serde_json::to_string(&Ident::from("test")).unwrap())
                .unwrap(),
            Ident::from("test")
        );
    }
}
