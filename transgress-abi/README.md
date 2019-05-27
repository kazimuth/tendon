# transgress-abi

A stable ABI for rust code.

## goals

- define ABI description format
  - text format for tool consumption
  - C API for usage in C
  - conversion between above
    - fully reversible?
    - just embed text format in executables?
- maximal ease of use
- some bloat is tolerable

use cases

- rust plugin system: load rust library dynamically, easy interop
- FFI: easily make insta-bindings to rust code based on ABI
- wasm-to-hardware calling-out?

## reqs / ideas

- capabilities?
- custom text format? serde?
- #[repr]s
- handle different memory allocators?
- dynamic linking?
  - priorities?
- allow multiple versions of single library
- allow as much cross-library-version usage as possible?
- handle generics through vtable ABI
  - need a vtable ABI
  - handle generics through snippets of generated rust code (nim)
- #[repr]s
- canonical types e.g. libc, std types
  - specs?
- define lowering from types to ABI description?
- handle multiple libraries with different options
  - library UUIDs? how would that interact w/ stacktraces?
  - library_hashes, like already used by rustc
  - require use of system allocator?
- handle type / trait compatibility between library versions
  - just fallback to semver?
- catch_panic
  - panic handling?
- identify crates by name+semver
  - additional checks?
- interaction with wasm cross-language efforts?

## impl

fast parsing + validation of ABIs

## reading

https://gankro.github.io/blah/rust-layouts-and-abis/

-> size, alignment

-> offsets within type: undefined unless transparent or C

-> reprs: Rust, C, transparent, packed(N), simd, align=X, int

https://doc.rust-lang.org/nomicon/README.html

-> all sortsa stuff

https://github.com/dtolnay/semver-trick
https://rust-lang-nursery.github.io/api-guidelines/future-proofing.html#future-proofing

-> special trick for handling cross-library

-> will be handled if we handle reexports correctly

https://docs.rs/abi_stable/0.4.1/abi_stable/
https://crates.io/crates/abi_stable

-> similar idea, but way more work / boilerplate than my goal

-> divides crate ABIs based on 0.x.0 and y.0.0

https://rustwasm.github.io/book/reference/which-crates-work-with-wasm.html

-> stuff that doesn't mess with the OS will run easy under wasm

https://github.com/WebAssembly/design/issues/1274

-> future?

https://en.wikipedia.org/wiki/GraalVM
