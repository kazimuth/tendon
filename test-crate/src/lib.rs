//! this is a test crate zoom zoom zoom

#![allow(unused, unused_variables)]

/// thing
pub fn x(y: i32) {}

/// opaque struct
pub struct Opaque {
    members: Vec<i32>,
}

/// non-opaque struct
/// hmm...
pub struct NonOpaque {
    pub members: Vec<i32>,
}

/// partially-opaque struct
pub struct PartiallyOpaque {
    pub members: Vec<i32>,
    _nonexhaustive: (),
}

pub struct Generic<T: Sized + std::io::Write> {
    pub generic_member: T,
    pub other: Opaque,
}

#[repr(C)]
pub struct ReprC {
    pub x: i32,
    pub y: *mut (),
}

pub mod z {
    pub struct InMod {}
}

pub use rand_chacha::ChaChaRng as ReexportedThing;

pub fn uses_other(z: rand_chacha::ChaChaCore) {}
