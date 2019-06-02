//! A dead-simple library for use with the new async/await.
//!
//! This library aims to have minimal dependencies and compile quickly.
//! It is intended for projects that can benefit from the mental model of futures but don't need a high-performance network runtime;
//! I wrote it to make work on a streaming compiler easier.

pub mod block;
pub mod once;

pub use block::unblock;
pub use once::once_future;
