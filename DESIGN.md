use cases

- large API surfaces that we don't want to bind by hand
- FFI: easily make insta-bindings to rust code based on ABI
- rust plugin system: load rust library dynamically

goals

- maximal ease of use
- some bloat is tolerable

### usage / user API ideas

- parse .d.rs-style definitions for missing things
  - maybe just a macro `transgress_metadata!` that can be invoked wherever, will be auto-harvested from dependent crates
  - makes it easy to make e.g. `transgress-uuid` crates or whatever to help out, ...
  - also, custom things in doc comments?
- build system integrations
  - automatically generate build files for multiple build systems
  - generate code for invoking cargo / transgress-rs from other build systems
    - bootstrap problem: write the minimum amount of code in all xyz languages to start executing rust
  - separate crates for different build systems (transgress-gradle, transgress-b
- custom conversions (convert numpy array to ndarray automatically?)

### [source-scrape](../transgress-source-scrape)

- only resolve public items as-needed until you have the full tree of things needed to access an API
- propagate resolution failures upwards to minimize the damage they do to an API
  - blacklists
  - unresolvable paths (due to unimplemented macros / #[cfg]s, etc)
  - parser failures
- lower APIs to simplified [format](#transgress-api) that's easy to serialize / parse

- metadata from closures can only leak to the typesystem through the automatic borrows + send + sync on impl Trait

  - solution: treat impl Trait as non-send + non-sync
    - can get fancier later, talk to analysis or sth
  - impl Trait borrows?

- stupid rustc query system

  - build a program that asks the compiler a bunch of questions and prints the answers
    - type sizes and alignments
    - constexpr resolution

- coherence rules?
  - https://github.com/rust-lang/rfcs/blob/master/text/1023-rebalancing-coherence.md
- chalk:
  - https://rust-lang.github.io/rustc-guide/traits/chalk-overview.html
  - https://github.com/rust-analyzer/rust-analyzer/search?q=chalk&unscoped_q=chalk
- proc-macro-expander: https://github.com/fedochet/rust-proc-macro-expander
  - can be distributed w/ prefab?
    assuming
  - use a recent required rust version to solve API compat issues?
    - or just disable on older rust

https://doc.rust-lang.org/stable/reference/lifetime-elision.html

algorithm:

```
[too much at once. rather, start small and build up as we go. add "could not resolve" errors that you can solve
by hand.]

load base crate in full. find paths to cargo and rust-src.
for each macro in crate:
    expand macro
for each exported item in crate:
    resolve all contained paths.
for each exported type in crate:
    determine send + sync from composition.
for each exported trait:
    find all impls of that trait.
    for all types and traits referenced from impls:
        find all impls of those too.
            orphan rules mean fancy impls must be local to defining crate;
            other impls must be on foreign types.
            [send + sync?]
            [can we feed chalk partial information here?]
    for each exported type:
        use chalk to resolve exported traits for that type.
            [what about references, boxes, pins, etc?]
            [is there a "search rust code" thing somewhere?]

resolve path:
    lookup path in module context.
    if path is in crate, done.
    if path resolves to unloaded crate:
        load that crate in full.

load crate in full:
    find crate source. parse all source files, expanding macros as we go.
        macro name resolution?
        mbe: done
        #[derive]: for derive-safe types, just generate empty impl?
            no: vulkano shaders, &ct
            can have whitelist tho
        for attributes, non-mbes, non-derive-safe types:
            run the corresponding proc-macros

determine send + sync from composition:
    if you're a primitive type, stop.
    if there's a chalk impl for you, use chalk.
    look up all members. determine send + sync from composition.
    [do we need chalk here?]

[N.b. export rules?]
    [what about pub impls / standard traits?]
    [what about referenced paths, e.g. types used in function sigs but not exported?]
        [try some different options here + customize + override]

[how to handle no-having-multiple-deps-with-same-version req?]
    [generate sub-wrapper crates for every version, lomarf]
    [disable lower versions]

```

data structures:

```
DepGraph {
    // crate locations, dep graphs, features, etc.
}

Db {
    path: api::Path => Item {
            MacroDef {}
            MacroCall {}
            StructDef {}
            TraitDef {}
            EnumDef {}
            ReexportDef {}
            ...
        }
    }
]

CrateQueue [ api::CrateRef ]

Scope {
    parent: Option<Arc<Scope>>>
    globs: [Glob { path }]
    entries: [Ident => Path]
}

loop {
    for crate in ctx.crateQueue.take().par_drain() {
        crate.parse(ctx);
    }
    for macro in unexpanded_macros.take().par_drain() {
        macro.expand(ctx);
    }
    for element in unresolved_elements.take().par_drain() {
        // puts elt back if it's not used
        element.resolve(ctx);
    }
}

how do attribute macros get threaded through?

this is just futures still

scope.await

i don't know what interface the codegen crates will want to consume yet...

each module has a list of imports, glob-imports


```

### [transgress-api](../transgress-api)

- Rust API description format
  - text format for tool consumption
  - C API for usage in C
  - conversion between above
    - fully reversible?
    - just embed text format in executables?

### general FFI

see also: [runtime](#runtime)
can provide a lot of this as reusable components between impls
per-language impls can do stuff via whatever and use whatever components they need, keep it lightweight + invisible to end user

- repr(C) / C-compatible types
  - pass directly?
    - optionally?
    - struct / nostruct versions of API?
- send+sync handling
  - thread locals wrappers?
    - ThreadLocal<Option<T>>, fails if accessed from another thread; TODO benchmark thread.current().id()
- catch_panic
- auto-instantiation

  - default types
  - used in docs?

- runtime for support of fancy stuff

  - traits
  - borrows
    - implicit locks
      - context managers
      - locks last until GC
      - locks last until force-unlocked, after they're invalidated
        - requires a list of items using lock tho, since their destructors can do arbitrary things & need access to borrow state
        - yeesh
  - minimal-allocation iterators

- fast binding mode

  - skip:
    - utf-8 well-formedness checks, since the source language spec'd utf-8
    -

- fuzz testing?

- auto-flattening of passed structs to primitives to avoid buffer allocations

- representation of different sides of the data?
- C FFI representation that can be lowered to rust OR C (or other stuff...)

- LTO: https://doc.rust-lang.org/rustc/linker-plugin-lto.html

http://swig.org/Doc3.0/SWIG.html
-> SWIG!
- works similarly to this, a full c++ compiler, with "typemaps" generating code for individual c++ types

https://nullprogram.com/blog/2018/05/27/
-> indirect calls w/ dlsym is slightly faster than standard indirect linking; only .25 of a ns tho, not important

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

- storing data behind pointers vs storing data in-line
  - cross-rust compatibility?
    - note: you must re-generate header files whenever you run this code!
      any versions of compiled with a different rustc are NOT abi-compatible!! this crate provides no ABI compatibility guarantees for generated code!!!!!!!!!!
    - or, opt-in generated code
  - see notes on opaque bytes from bindgen? going the other way tho

convert consts to statics

TODO: just don't have impls in functions, that ISN'T handled!!!!! why would you even do that anyway
    or do it later

TODO: investigate how rust handles multiple runtimes floating around, e.g. rust code loading python .so statically linked to rust

### ffi generation implementation

have a couple layers of IR:

```
          .rs
           |
          syn
           |
    transgress-api
    /      |      \
binding<->abi<->rust backend
|          |        |
.py        .h     .impl.rs
.java
.hpp
.swig >:)
```

can insert synthetic code at the transgress-api level (simple helpers for enums, etc.)
can scan transgress-api for bindable patterns (error enums, etc.)
can check abi for backwards-compatibility

ideally, binding implementors only need to look at api and abi -- rust backend is already done?

can also ignore abi layer / define your own: java <-> rust

idea: some sort of rule-based API rewriting system?

- self->self methods can be refactored to not move ownership (just reassign to target slot)
- Builder pattern can be converted to kwargs in some cases
- borrows that auto-drop

### pitch

Instantly bind your rust code from 7 languages

- no handwritten unsafe ffi code
- no listing every type and function to bind by hand: just `transgress` and you're done*
- generates code, not binaries, to keep your IDE and documentation tools working
- simple integrations with build systems

example: let's bind the excellent [uuid](https://crates.io/crates/uuid) library from python. [rust parsers good fast raggum fraggum]

```sh
$ transgress generate test_crate python-poetry ...
```

`lib.rs`:

```rust
pub use uuid;
```

```sh
$ poetry build
```

`test.py`:

```python
from test_crate import uuid

try:
    uuid = uuid.Uuid.parse_str("936DA01F9ABD4d9d80C702AF85C822A8")
    print(uuid.to_urn())
except uuid.ParseError as e:
    print("UUID parse error:", e)
except uuid.BytesError as e:
    print("UUID byte count error: expected {} bytes, found {}".format(e.expected(), e.found()))
```

```sh
$ python test.py
```

alright that's nice but now i need to parse UUIDs from my android app. No problem:

`test.java`

```java
import test_crate.uuid.Uuid;

public class UuidTest {
    public static void main(String[] args) {
        try {
            Uuid uuid = Uuid.parseStr("936DA01F9ABD4d9d80C702AF85C822A8");
            System.out.println(uuid.toUrn())
        } catch (Exception e) {
            System.out.println("Uuid parse error", e);
        }
    }
}
```

and c, ...

The language you want not supported? Write your own integration:

- a simple integration can be written in a few hundred lines of code.
- implement fancier transgress-rs binding features at your leisure. the more you implement, the better the generated API.

features:

- support for nearly all rust-language features
- ...
- ...

differences from:

- cbindgen: this does a lot more
- language binding systems: this isn't manual
- wasm-bindgen: this'll call it for you

...

distribution notes:

- users need rust compiler, or need to distribute binaries

faq:
is it fast?
    faster than whatever language you're using lmao. unless you're using c/c++, in which case, about the same speed,
    but safer. ffi calls involve a function call whatever lang you're using (unless you use lto).
    
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

maven: https://www.mojohaus.org/maven-native/native-maven-plugin/

gradle: https://docs.gradle.org/current/userguide/native_software.html

### python

just use cffi lmao

https://cffi.readthedocs.io/en/latest/

context managers: all objects! but particularly some
also, implicit rwlocks around all objects (bench? safety / footgun?)

https://github.com/getsentry/milksnake
-> builds?

how to reacharound from cffi
e.g. rust code runs python script via cpython api calls back into rust from transgress: how do we make this work?

poetry: https://poetry.eustace.io/

https://eev.ee/blog/2013/09/13/cython-versus-cffi/

### rust-rust

https://docs.rs/abi_stable/0.4.1/abi_stable/
https://crates.io/crates/abi_stable

-> similar idea, but way more work / boilerplate than my goal

-> divides crate ABIs based on 0.x.0 and y.0.0

https://github.com/rust-lang/rust/tree/master/src/libproc_macro/bridge

basically this but hand-implemented to allow cross-compiler proc-macro invocations
(why did they even do this? whatever, it's cool)

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

could use cargo to replace all lib dependencies w/ ABI-level things

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

### C/C++

see cbindgen notes for how they did this (only for ffi stuff tho, not general code)

option to only generate ffi-code, "cbindgen"

### bindgen notes

https://github.com/rust-lang/rust-bindgen

> When `bindgen` finds a type that is too difficult or impossible to translate
> into Rust, it will automatically treat it as an opaque blob of bytes. The
> philosophy is that
>
> 1. we should always get layout, size, and alignment correct, and
>
> 2. just because one type uses specialization, that shouldn't cause `bindgen` to
>    give up on everything else.
>
> Without further ado, here are C++ features that `bindgen` does not support or
> cannot translate into Rust:
>
> - Inline functions and methods: see
>   ["Why isn't `bindgen` generating bindings to inline functions?"](./faq.md#why-isnt-bindgen-generating-bindings-to-inline-functions)
>
> - Template functions, methods of template classes and structs. We don't know
>   which monomorphizations exist, and can't create new ones because we aren't a
>   C++ compiler.
>
> - Anything related to template specialization:
>
>   - Partial template specialization
>   - Traits templates
>   - Specialization Failure Is Not An Error (SFINAE)
>
> - Cross language inheritance, for example inheriting from a Rust struct in C++.
>
> - Automatically calling copy and/or move constructors or destructors. Supporting
>   this isn't possible with Rust's move semantics.
>
> - Exceptions: if a function called through a `bindgen`-generated interface
>   raises an exception that is not caught by the function itself, this will
>   generate undefined behaviour. See
>   [the tracking issue for exceptions](https://github.com/rust-lang/rust-bindgen/issues/1208)
>   for more details.

### cbindgen notes

https://github.com/eqrion/cbindgen
https://blog.eqrion.net/future-directions-for-cbindgen/

has taken a similar approach to this crate - has similar problems with finding names + macros
might be able to collab on harvesting types and stuff, tho of course cbindgen would only care about ffi types
does not generate tests

https://github.com/eqrion/rust-ffi: links to rustc nightly to directly harvest ffi information, prototype

example ffi.json: https://gist.github.com/eqrion/c15361006039e369b0c7a3d9b19a08d7

https://github.com/eqrion/cbindgen/blob/master/docs.md

> While modules within a crate form a tree with uniquely defined paths to each item, and therefore uniquely defined cfgs for those items, dependencies do not. If you depend on a crate in multiple ways, and those ways produce different cfgs, one of them will be arbitrarily chosen for any types found in that crate.

https://github.com/eqrion/cbindgen/blob/master/docs.md#supported-types :

> Most things in Rust don't have a guaranteed layout by default. In most cases this is nice because it enables layout to be optimized in the majority of cases where type layout isn't that interesting. However this is problematic for our purposes. Thankfully Rust lets us opt into guaranteed layouts with the `repr` attribute.
>
> You can learn about all of the different repr attributes [by reading Rust's reference][reference], but here's a quick summary:
>
> - `#[repr(C)]`: give this struct/union/enum the same layout and ABI C would
> - `#[repr(u8, u16, ... etc)]`: give this enum the same layout and ABI as the given integer type
> - `#[repr(transparent)]`: give this single-field struct the same ABI as its field (useful for newtyping integers but keeping the integer ABI)
>
> cbindgen does not currently support the align or packed reprs.
>
> However it _does_ support using `repr(C)`/`repr(u8)` on non-C-like enums (enums with fields). This gives a C-compatible tagged union layout, as [defined by this RFC 2195][https://github.com/rust-lang/rfcs/blob/master/text/2195-really-tagged-unions.md]. `repr(C)` will give a simpler layout that is perhaps more intuitive, while `repr(u8)` will produce a more compact layout.
>
> If you ensure everything has a guaranteed repr, then cbindgen will generate definitions for:
>
> - struct (named-style or tuple-style)
> - enum (fieldless or with fields)
> - union
> - type
> - `[T; n]` (arrays always have a guaranteed C-compatible layout)
> - `&T`, `&mut T`, `*const T`, `*mut T`, `Option<&T>`, `Option<&mut T>` (all have the same pointer ABI)
> - `fn()` (as an actual function pointer)
> - `bitflags! { ... }` (if macro_expansion.bitflags is enabled)
>
> structs, enums, unions, and type aliases may be generic, although certain generic substitutions may fail to resolve under certain configurations. In C mode generics are resolved through monomorphization and mangling, while in C++ mode generics are resolved with templates. cbindgen cannot support generic functions, as they do not actually have a single defined symbol.
>
> cbindgen sadly cannot ever support anonymous tuples `(A, B, ...)`, as there is no way to guarantee their layout. You must use a tuple struct.
>
> cbindgen also cannot support wide pointers like `&dyn Trait` or `&[T]`, as their layout and ABI is not guaranteed. In the case of slices you can at least decompose them into a pointer and length, and reconstruct them with `slice::from_raw_parts`.
>
> If cbindgen determines that a type is zero-sized, it will erase all references to that type (so fields of that type simply won't be emitted). This won't work if that type appears as a function argument because C, C++, and Rust all have different definitions of what it means for a type to be empty.
>
> Don't use the `[u64; 0]` trick to over-align a struct, we don't support this.
>
> cbindgen contains the following hardcoded mappings (again completely ignoring namespacing, literally just looking at the name of the type):
>
> - bool => bool
> - char => wchar_t
> - u8 => uint8_t
> - u16 => uint16_t
> - u32 => uint32_t
> - u64 => uint64_t
> - usize => uintptr_t
> - i8 => int8_t
> - i16 => int16_t
> - i32 => int32_t
> - i64 => int64_t
> - isize => intptr_t
> - f32 => float
> - f64 => double
> - VaList => va_list
> - PhantomData => _evaporates_, can only appear as the field of a type
> - () => _evaporates_, can only appear as the field of a type
>
> - c_void => void
> - c_char => char
> - c_schar => signed char
> - c_uchar => unsigned char
> - c_float => float
> - c_double => double
> - c_short => short
> - c_int => int
> - c_long => long
> - c_longlong => long long
> - c_ushort => unsigned short
> - c_uint => unsigned int
> - c_ulong => unsigned long
> - c_ulonglong => unsigned long long
>
> - uint8_t => uint8_t
> - uint16_t => uint16_t
> - uint32_t => uint32_t
> - uint64_t => uint64_t
> - uintptr_t => uintptr_t
> - size_t => size_t
> - int8_t => int8_t
> - int16_t => int16_t
> - int32_t => int32_t
> - int64_t => int64_t
> - intptr_t => intptr_t
> - ssize_t => ssize_t
> - ptrdiff_t => ptrdiff_t

### libffi notes

https://github.com/libffi/libffi

### julia

Best option: c lib wrapper generator: https://github.com/JuliaInterop/Clang.jl

Other options:
https://docs.julialang.org/en/v1/manual/calling-c-and-fortran-code/index.html
https://github.com/JuliaInterop/CxxWrap.jl
https://github.com/JuliaInterop/Cxx.jl

### cargo resolution

https://github.com/rust-lang/cargo/blob/37cb9bbe2428e8d591d42673ef5562ca3ca92c55/src/cargo/core/resolver/mod.rs

### erlang

https://github.com/rusterlium/rustler