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
- fuzz testing?

use cases

- large API surfaces that we don't want to bind by hand
- FFI: easily make insta-bindings to rust code based on ABI
- rust plugin system: load rust library dynamically

## reading / ideas

### general FFI

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

- runtime for support of fancy stuff
  - traits
  - borrows
  - minimal-allocation iterators

http://swig.org/Doc3.0/SWIG.html
-> SWIG!

https://nullprogram.com/blog/2018/05/27/
-> indirect calls w/ dlsym is slightly faster than standard indirect linking; only .25 of a ns tho, not important
for our purposes

https://hacks.mozilla.org/2019/04/crossing-the-rust-ffi-frontier-with-protocol-buffers/
-> protocol buffers are better than e.g. json for serializing across an ffi boundary; reasonably fast, very stable

https://crates.io/crates/ffi-support
-> https://docs.rs/ffi-support/0.3.4/ffi_support/handle_map/index.html: dynamically checked FFI pointers
-> expose a transparent version of this by hashing returned pointers in debug mode?
-> https://docs.rs/ffi-support/0.3.4/ffi_support/macro.static_assert.html: simple static_assert! macro
-> macros to specify different protocols for passing things thru the FFI boundary
-> https://docs.rs/ffi-support/0.3.4/ffi_support/struct.FfiStr.html: #[repr(transparent)] type for c-strings
-> https://docs.rs/ffi-support/0.3.4/ffi_support/trait.IntoFfi.html: impl of through-FFI stuff as a rust trait

https://github.com/mozilla/application-services/blob/master/docs/product-portal/applications/consuming-megazord-libraries.md
-> combine lots of rust code into a single library

https://blog.sentry.io/2016/10/19/fixing-python-performance-with-rust/

https://gankro.github.io/blah/rust-layouts-and-abis/

-> size, alignment

-> offsets within type: undefined unless transparent or C

-> reprs: Rust, C, transparent, packed(N), simd, align=X, int

https://doc.rust-lang.org/nomicon/README.html

-> all sortsa stuff

https://github.com/dtolnay/semver-trick
https://rust-lang-nursery.github.io/api-guidelines/future-proofing.html#future-proofing

-> special trick for handling cross-library deps

-> will be handled if we handle reexports correctly

### wasm

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

https://rustwasm.github.io/docs/wasm-bindgen/reference/arbitrary-data-with-serde.html
-> serde to alt. object model

### java / android

https://docs.oracle.com/javase/7/docs/technotes/guides/jni/spec/jniTOC.html
-> JNI spec, inc. name mangling

https://github.com/jnr/jnr-ffi
-> dead-simple JNI w/o C code

https://docs.rs/jni/0.12.3/jni/
-> good rust lib for JNI

https://developer.android.com/training/articles/perf-jni
-> JNI tips on android
-> NDK is just JNI w/ some extra stuff

https://jdk.java.net/panama/
-> future JNI system for ffi

### python

just use cffi lmao

https://cffi.readthedocs.io/en/latest/

### rust-rust

https://docs.rs/abi_stable/0.4.1/abi_stable/
https://crates.io/crates/abi_stable

-> similar idea, but way more work / boilerplate than my goal

-> divides crate ABIs based on 0.x.0 and y.0.0

### runtime

can provide code that does optional validation; basically a helper crate for implementing different generators. generators can use as much or as little of it as they like

- custom text format
  - serde
  - just rust signatures?
    - in some limited form?
    - can be an input, at least
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
  - require the system allocator?

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

yes -- a general rust ABI is actually very hard

- are we actually making an ABI? it's sorta in-between, right?
  - https://en.wikipedia.org/wiki/Application_binary_interface
  - we define a C API that's guaranteed to be ABI-backwards-compatible (to some extent) on a single target triple
    - different target triples are incompatible
  - we depend on the system ABI.
- ABI:

building custom bindings for each language that don't go through a particular API

- auto-impl coercion for some types
  - arrays, maps
  - numbers
  - strings

### inter-ffi linking

use multiple rust-based libraries in something?

solution: just don't solve this, lmao

- across-library usage might not be very common
- use case: plugins
  - plugins talk to host, not much to each other?
- use case: multiple ffi libraries
  - can try deserialization from host data model
  - can try tagging structs with metadata?
    - ... hash ...
    - repr(C) special case
  - jitting / caching decisions at boundary
- custom metadata for stable things?
- can use via-C API from both libs
