//! A dead-simple library for use

pub mod block;
pub mod once;

pub use block::unblock;
pub use once::once_future;
mod fs;
