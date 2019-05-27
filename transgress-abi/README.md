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

## reqs / ideas

abi

- repr(C) / C-compatible types
  - pass directly?
    - optionally?
    - struct / nostruct versions of API?
- send+sync handling
  - thread locals wrappers?
    - ThreadLocal<Option<T>>
- catch_panic
- auto-instantiation
  - default types
  - used in docs?

runtime

- custom text format
  - serde
  - just rust signatures?
- #[repr]s
- canonical types e.g. libc, std types
- dynamic linking
  - support loading multiple modules in this style, calling into each other
  - rust runtime
- handle generics through vtable ABI
  - need a vtable ABI
  - handle generics through snippets of generated rust code (nim)
- define lowering from types to ABI description?
  - c.f. IntoWasmABI
- handle multiple libraries with different options
  - identify crates by name+semver (+ option hash?)
  - additional checks?
- reexports
  serde?
- capabilities?

* provide lowering to wasm-bindgen from rustdoc-scrape / from ABI

  - autobindgen

* incoming: provide rustlike ABI for code in other language

  - define a c header and let language impl that

* wasm-to-hardware calling-out?
  - ABI across wasm boundary?
  - what setting would use this?
    - wasm embedding in tooling
    - eh

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
https://rustwasm.github.io/2018/07/02/vision-for-wasm-bindgen.html

-> JS / web stuff is hell and i don't want to deal with it

https://github.com/rustwasm/wasm-bindgen/blob/master/crates/backend/src/ast.rs

-> AST for wasm ABI

-> uses syn types

-> handles both incoming and outgoing FFI, and has different types for each

https://rustwasm.github.io/wasm-bindgen/api/wasm_bindgen/convert/trait.IntoWasmAbi.html
https://github.com/rustwasm/wasm-bindgen/blob/master/src/convert/impls.rs

http://swig.org/Doc3.0/SWIG.html
