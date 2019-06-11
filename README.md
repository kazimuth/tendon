# rust-swig but Worse

this crate is a work-in-progress and currently only contains README files

automatically generate an [ABI](https://github.com/kazimuth/transgress-rs/tree/master/transgress-abi) /
FFI API for rust crates

TODO:

- needs MIT license
- check parser against older rls-data formats
- FIXMEs in https://github.com/rust-lang/rust/blob/c22ce2806c3/src/librustc_save_analysis/sig.rs are preventing good
  signature data for types >:(
- https://rust-lang.github.io/rustc-guide/about-this-guide.html

ideally:

- magically create ABI over top of ABI-less rust
- use from multiple languages
- not require hand-annotation

use-cases:

aimed at large API surfaces that we don't want to bind by hand

- stable rust plugins
- python bindings
- java bindings
- nim / c / c++ bindings
- wasm bindings

impl strategies:

- get API surface

  - rustdoc path

    - handles macro-expansion, resolve, ... for us
    - just parse rustdoc's output, lol
    - https://github.com/servo/html5ever

  - other paths: rust-analyzer, rustc nightly, hand implementation, ...
    - rlibs

https://github.com/rust-lang/rust/issues/25820
