# api-scrape

Looks at a crate's source and Cargo.toml to find the crate's public API.
Best-effort: proc-macros may not work?

- only resolve public items as-needed until you have the full tree of things needed to access an API
- propagate resolution failures upwards to minimize the damage they do to an API
  - blacklists
  - unresolvable paths (due to macros / #[cfg]s, etc)
  - parser failures
  - this can be done with futures
- lower APIs to simplified format that's easy to serialize / parse; can be cached + distributed
- also parse .d.rs-style definitions for missing things
- metadata from closures can only leak to the typesystem through the automatic borrows + send + sync on impl Trait
  - solution: treat impl Trait as non-send + non-sync
    - can get fancier later, talk to analysis or sth
  - impl Trait borrows?
- coherence rules?
  - https://github.com/rust-lang/rfcs/blob/master/text/1023-rebalancing-coherence.md
- chalk:
  - https://rust-lang.github.io/rustc-guide/traits/chalk-overview.html
  - https://github.com/rust-analyzer/rust-analyzer/search?q=chalk&unscoped_q=chalk

algorithm:

```
[too much at once. rather, start small and build up as we go. add "could not resolve" errors that you can solve
by hand.]

load base crate in full. find paths to cargo and rust-src.
for each item-macro in crate:
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

```
