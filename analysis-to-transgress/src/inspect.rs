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
        write!(
            out,
            "#[{}] {} {}",
            self.attributes.len(),
            kind,
            self.qualname,
        )?;
        if let Some(ref sig) = self.sig {
            write!(out, " [{}]", sig.text)?;
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
