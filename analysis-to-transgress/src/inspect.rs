#![allow(unused_must_use)]

use rls_data::DefKind;
use std::fmt;

pub trait Inspect {
    fn inspect(&self, out: &mut fmt::Formatter) -> Result<(), fmt::Error>;
}

impl Inspect for rls_data::Def {
    fn inspect(&self, out: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let kind = match self.kind {
            DefKind::Const => "const",
            DefKind::Enum => "enum",
            DefKind::ExternType => "extern_type",
            DefKind::Field => "field",
            DefKind::ForeignFunction => "foreign_function",
            DefKind::ForeignStatic => "foreign_static",
            DefKind::Function => "function",
            DefKind::Local => "local",
            DefKind::Macro => "macro",
            DefKind::Method => "method",
            DefKind::Mod => "mod",
            DefKind::Static => "static",
            DefKind::Struct => "struct",
            DefKind::StructVariant => "struct_variant",
            DefKind::Trait => "trait",
            DefKind::Tuple => "tuple",
            DefKind::TupleVariant => "tuple_variant",
            DefKind::Type => "type",
            DefKind::Union => "union",
        };
        if let Some(ref sig) = self.sig {
            print_sig(sig, out);
        } else {
            //write!(out, "{} {}", kind, self.qualname)?;
        }
        Ok(())
    }
}

pub struct Inspected<'a, T: Inspect>(pub &'a T);
impl<T: Inspect> fmt::Display for Inspected<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.0.inspect(f)
    }
}

static COLORS: &'static [&'static str] = &["\x1b[0m", "\x1b[34m", "\x1b[35m", "\x1b[36m"];
struct Colorstack(Vec<usize>, usize);
impl Colorstack {
    fn new() -> Self {
        Colorstack(vec![0], 1)
    }
    fn last(&self) -> usize {
        self.0[self.0.len() - 1]
    }
    fn push(&mut self, f: &mut fmt::Formatter) {
        self.1 += 1;
        if self.1 >= COLORS.len() {
            self.1 = 1;
        }
        self.0.push(self.1);
        write!(f, "{}", COLORS[self.last()]);
    }
    fn pop(&mut self, f: &mut fmt::Formatter) {
        self.0.pop();
        write!(f, "{}", COLORS[self.last()]);
    }
}

fn print_sig(sig: &rls_data::Signature, f: &mut fmt::Formatter) {
    let mut stack = Colorstack::new();
    let p = format!("{} ", sig.text);

    for (i, c) in p.chars().enumerate() {
        for def in &sig.defs {
            if def.start == i {
                stack.push(f);
            }
            if def.end == i {
                stack.pop(f);
            }
        }
        for def in &sig.refs {
            if def.start == i {
                stack.push(f);
            }
            if def.end == i {
                stack.pop(f);
            }
        }
        if i < sig.text.len() {
            write!(f, "{}", c);
        }
    }
}
pub struct Sig {}
