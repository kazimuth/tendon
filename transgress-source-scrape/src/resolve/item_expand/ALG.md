{ $($root:ident [$($child:ident)+])+ $($after:lit)+} => { $($root $after [$(\$child)+])+ }

A [b c d] E [f g h] 5 10
A 5 [b c d] E 10 [f g h]

for each group:

- create group node
- attach child groups to this group node

```
r:A           r:E          r2:A2              r2:E2
c:b c:c c:d   c:f c:g c:h  c2:b2 c2:c2 c2:d2  c2:b2 c2:c2 c2:d2
```

for each transcription group:

- get pointer to all matched groups
  - what about no matchers?
- step groups
- for child groups:
  - select pointers based on children of current parent groups

matchers only bind once
matchers only bind at their depths

node: matcher bindings + children

```rust
enum NamedMatch {
    Seq(Vec<NamedMatch>),
    Match(TokenTree)
}

struct Bindings(HashMap<Ident, NamedMatch>);


```

https://github.com/rust-lang/rust/blob/master/src/libsyntax/ext/tt/transcribe.rs
