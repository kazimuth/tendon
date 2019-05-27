//! this is a test crate zoom zoom zoom

// thing
pub fn x(y: i32) {}

pub struct Opaque {
    members: Vec<i32>,
}

pub struct Generic<T> {
    generic_member: T,
}

#[repr(C)]
pub struct ReprC {
    x: i32,
    y: *mut (),
}

pub use rand_chacha::ChaChaRng as ReexportedThing;
