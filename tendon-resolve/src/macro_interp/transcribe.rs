use super::{ast, consume::Binding};
use crate::{Map, Set};
use proc_macro2 as pm2;
use quote::{quote, ToTokens};
use std::mem;
use syn::spanned::Spanned;
use tracing::warn;

pub fn transcribe(
    bindings: &Map<String, Binding>,
    rule: &ast::TranscribeSeq,
) -> syn::Result<pm2::TokenStream> {
    let mut output = pm2::TokenStream::new();
    {
        let mut ctx = Ctx {
            bindings,
            output: &mut output,
            repetition_stack: vec![],
        };
        rule.transcribe(&mut ctx)?;
    }

    Ok(output)
}

struct Ctx<'a> {
    /// The bindings we have access to.
    bindings: &'a Map<String, Binding>,
    /// The (current) output stream we're writing to.
    output: &'a mut pm2::TokenStream,
    /// Where we are within the stack of repetitions.
    repetition_stack: Vec<usize>,
}
impl Ctx<'_> {
    fn write(&mut self, item: &dyn ToTokens) {
        item.to_tokens(self.output)
    }
}

trait Transcriber {
    fn transcribe(&self, ctx: &mut Ctx) -> syn::Result<()>;
}
impl Transcriber for ast::Transcribe {
    fn transcribe(&self, ctx: &mut Ctx) -> syn::Result<()> {
        match self {
            ast::Transcribe::Fragment(fragment) => fragment.transcribe(ctx)?,
            ast::Transcribe::Repetition(repetition) => repetition.transcribe(ctx)?,
            ast::Transcribe::Group(group) => group.transcribe(ctx)?,
            ast::Transcribe::Ident(ident) => ctx.write(ident),
            ast::Transcribe::Literal(literal) => ctx.write(literal),
            ast::Transcribe::Punct(punct) => ctx.write(punct),
        }
        Ok(())
    }
}

impl Transcriber for ast::TranscribeSeq {
    fn transcribe(&self, ctx: &mut Ctx) -> syn::Result<()> {
        for t in &self.0 {
            t.transcribe(ctx)?;
        }
        Ok(())
    }
}

impl Transcriber for ast::TranscribeGroup {
    fn transcribe(&self, ctx: &mut Ctx) -> syn::Result<()> {
        // store current output on the stack, create a new output for within this group
        let mut output = pm2::TokenStream::new();
        mem::swap(ctx.output, &mut output);

        // run code
        let inner = self.inner.transcribe(ctx);

        // restore state
        mem::swap(ctx.output, &mut output);

        // check errors
        inner?;

        ctx.write(&proc_macro2::Group::new(self.delimiter, output));
        Ok(())
    }
}
impl Transcriber for ast::TranscribeRepetition {
    fn transcribe(&self, ctx: &mut Ctx) -> syn::Result<()> {
        let fragments = find_level_fragments(self);

        let mut reps = None;

        for frag in fragments {
            if let Some(binding) = ctx.bindings.get(frag) {
                let current_repetition = binding.get(&ctx.repetition_stack[..]);
                if let Some(Binding::Seq(current_repetition)) = current_repetition {
                    if let Some(reps) = reps {
                        if reps != current_repetition.len() {
                            return Err(syn::Error::new(
                                quote!(_).span(),
                                "mismatched repetition count",
                            ));
                        }
                    } else {
                        reps = Some(current_repetition.len())
                    }
                }
            }
            // else: no binding: we just transcribe the fragment specifier verbatim
        }
        let reps = if let Some(reps) = reps {
            reps
        } else {
            // nothing to do
            return Ok(());
        };

        for i in 0..reps {
            ctx.repetition_stack.push(i);
            let ok = self.inner.transcribe(ctx);
            ctx.repetition_stack.pop();
            if i < reps - 1 {
                for sep in &self.sep.0 {
                    ctx.write(sep);
                }
            }
            ok?;
        }

        Ok(())
    }
}

fn find_level_fragments(rep: &ast::TranscribeRepetition) -> Set<&str> {
    let mut fragments = Set::default();
    fn find_inner<'a>(transcribe: &'a ast::Transcribe, fragments: &mut Set<&'a str>) {
        match transcribe {
            ast::Transcribe::Fragment(fragment) => {
                fragments.insert(&fragment.0);
                ()
            }
            ast::Transcribe::Group(group) => {
                for item in &group.inner.0 {
                    find_inner(item, fragments);
                }
            }
            _ => (),
        }
    }
    for item in &rep.inner.0 {
        find_inner(item, &mut fragments);
    }
    fragments
}
impl Transcriber for ast::TranscribeFragment {
    fn transcribe(&self, ctx: &mut Ctx) -> syn::Result<()> {
        if let Some(binding) = ctx.bindings.get(&self.0) {
            if let Some(Binding::Leaf(tokens)) = binding.get(&ctx.repetition_stack[..]) {
                ctx.write(&tokens);
                return Ok(());
            } else {
                warn!("binding at wrong level to transcribe: {}", &self.0);
            }
        }
        // no binding found, transcribe directly
        ctx.write(&quote!($));
        ctx.write(&pm2::Ident::new(&self.0, quote!(_).span()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::consume::Stomach;
    use super::ast::MacroDef;
    use super::*;

    #[test]
    fn full_macro() {
        let rules = quote! { macro_rules! test_macro {
            ($($x:ident $y:ident),+) => ([$($x)+] [$($y)+]);
        }};
        let rules = syn::parse2::<MacroDef>(rules).unwrap();

        let mut stomach = Stomach::new();

        let input = quote!(a b, c d, e f);

        stomach.consume(&input, &rules.rules[0].matcher).unwrap();

        let output = transcribe(&stomach.bindings, &rules.rules[0].transcriber).unwrap();

        assert_eq!(output.to_string(), quote!([a c e] [b d f]).to_string());
    }
}
