#![allow(unused)]

use rls_data::{Id, SigElement, Signature};
use std::fmt::Write;

/// Easily build up a signature recursively.
struct SigBuilder {
    text: String,
    refs: Vec<SigElement>,
    defs: Vec<SigElement>,
}

impl SigBuilder {
    fn new() -> SigBuilder {
        SigBuilder {
            text: String::new(),
            refs: Vec::new(),
            defs: Vec::new(),
        }
    }
    fn build(self) -> Signature {
        let SigBuilder { refs, defs, text } = self;
        Signature { refs, defs, text }
    }
}
macro_rules! text {
    ($builder:expr, $rest:tt) => {
        write!(&mut $builder.text, $rest)
            .ok()
            .ok_or("write failed")?
    };
}
macro_rules! ref_ {
    ($builder:expr, $id:expr, $($rest:expr),+) => {{
        let start = $builder.text.len();
        let _ = write!(&mut $builder.text, $($rest),+);
        let end = $builder.text.len();
        $builder.refs.push(SigElement {
            start,
            end,
            id: $id,
        });
    }};
}
macro_rules! def_ {
    ($builder:expr, $id:expr, $($rest:expr),+) => {{
        let start = $builder.text.len();
        write!(&mut $builder.text, $($rest),+)
            .ok()
            .ok_or("write failed")?;
        let end = $builder.text.len();
        $builder.defs.push(SigElement {
            start,
            end,
            id: $id,
        });
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format() -> Result<(), &'static str> {
        let mut b = SigBuilder::new();
        let id = Id { krate: 0, index: 0 };
        text!(&mut b, "struct ");
        def_!(&mut b, id, "Bees");
        text!(&mut b, "<");
        def_!(&mut b, id, "T");
        text!(&mut b, ": ");
        ref_!(&mut b, id, "{}", "Honey");
        text!(&mut b, "> {{}}");
        let result = b.build();
        assert_eq!(result.text, "struct Bees<T: Honey> {}");
        assert_eq!(result.defs.len(), 2);
        assert_eq!(result.refs.len(), 1);
        assert_eq!(
            &result.text[result.defs[0].start..result.defs[0].end],
            "Bees"
        );
        assert_eq!(&result.text[result.defs[1].start..result.defs[1].end], "T");
        assert_eq!(
            &result.text[result.refs[0].start..result.refs[0].end],
            "Honey"
        );
        Ok(())
    }
}
