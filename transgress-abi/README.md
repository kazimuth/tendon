# transgress-abi

A stable ABI for rust code?

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

- rust plugin system: load rust library dynamically
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
  - support loading multiple modules in this style, calling into each other?
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
- allocators
  - https://doc.rust-lang.org/1.9.0/book/custom-allocators.html#default-allocator
  - require the system allocator

* provide lowering to wasm-bindgen from rustdoc-scrape / from ABI

  - autobindgen

* incoming: provide rustlike ABI for code in other language

  - define a c header and let language impl that

* wasm-to-hardware calling-out?
  - ABI across wasm boundary?
  - what setting would use this?
    - wasm embedding in tooling
    - eh

compare: cffi, abi-vs-api bindings

1. lower to ABI

- generate rust shim implementing ABI
  - mask ABI-less things
- generate [target-lang] shim using ABI

2. OR, generate rust code that generates API?

- wasm-bindgen
- rust-swig

is there a difference?

yes! -- a general rust ABI is actually very hard!

- are we actually making an ABI? it's sorta in-between, right?
  - https://en.wikipedia.org/wiki/Application_binary_interface
  - we define a C API that's guaranteed to be ABI-backwards-compatible (to some extent) on a single target triple
    - different target triples are incompatible
  - we depend on the system ABI.
- ABI:

building custom bindings for each language that don't go through a particular API

- use multiple rust-based libraries in something?

  - rust libraries would need to be dynamically linked
  - https://doc.rust-lang.org/reference/linkage.html
    - dylibs exist, but don't work across compiler versions
  - _might_ be able to instrument libraries to be dynamically linked, but: hard.
    - patches: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html

okay, so don't need to dynamically link all rust libraries, but want efficient interchange of data through FFI?

e.g. ndarray passed to rust library using ndarray without conversion:

`pip install ndarray-rs arraything-rs`

`arraything.arraything(ndarray.make_array())`

ndarray (rust) -> python wrapper -> arraything (rust)

wrapping and unwrapping stages defined to be invertible?

- this is actually required for any rust-rust dynamic-calling

special-case for same compiler version + lib hash?

- might technically be undefined

okay so then: ABI is essentially a high-performance serialization schema

-> ...for data interchange only.

but: we don't have serde for all types.

opaque pointer design: simple to implement, but doesn't compose across libraries

mix of opaque pointers + serialization?
opaque by default + serialization where needed?

is linking two libraries with the same semver major version but different compiler versions supported?

what about compatible types across semver versions?
-> must use semver trick?
-> annotations / docs?
-> `#[since]` annotation?

two systems: ffi bindings and rust-rust calling

- ffi bindings use rust ...

- serializable types

- want a .d.ts style system -- mixins for uncontrolled libs; but interacts weirdly w/ soundness rules
  - impl Transgressor<NDarray> for NDArrayTransgressor
  - we can find this because we have the whoooole API to look at, owo
    - or just throw it in a macro to be exported idk
      - `load_transgressors! { helpers_a, [default-loaded helper lib[s]] };`
    - we're already breaking the rules lol

```rust
let q: Remote<Thing> = dynload().new_thing();

q.method(1i32, s: serde::Serialize)
```

is serde-serializability a semver guarantee?

- likely not considered breaking changes: reordering fields, adding private fields?
- so, maybe not?
- try anyway?
  - best-effort cross-library integration?

bincode serialization for SpEeEd

- likely faster than FFI calls for every serde:: call

problem is we have a lotta copies of different things floating around

alt-solution: just don't solve this, lmao

- across-library usage might not be very common
- use case: plugins
  - plugins talk to host, not much to each other?
- use case: multiple ffi libraries
  - can try deserialization from host data model
  - can try tagging structs with metadata?
    - ... hash ...
    - repr(C) special case
  - jitting / caching decisions at boundary
- custom metadata for stable things

use case: large API surfaces that we don't want to bind by hand

sample libraries:

- auto-impl coercion for some types
  - arrays, maps
  - numbers
  - strings

fighting things:

-

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

https://rustwasm.github.io/docs/wasm-bindgen/reference/arbitrary-data-with-serde.html
