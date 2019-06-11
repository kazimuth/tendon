# api-scrape

Looks at a crate's source and Cargo.toml to find the crate's public API. Best-effort.

- only resolve public items as-needed until you have the full tree of things needed to access an API
- propagate resolution failures upwards to minimize the damage they do to an API
  - blacklists
  - unresolvable paths (due to macros / #[cfg]s, etc)
  - parser failures
  - this can be done with futures
- lower APIs to simplified format that's easy to serialize / parse; can be cached + distributed
- also parse .d.rs-style definitions for missing things
