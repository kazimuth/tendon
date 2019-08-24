// TODO how to inject syn invisible parens?

use super::{ast, consume::Binding};
use crate::{Map, Set};
use proc_macro2 as pm2;
use quote::ToTokens;
use std::mem;

struct Ctx<'a> {
    bindings: &'a Map<String, Binding>,
    output: &'a mut pm2::TokenStream,
}
impl Ctx<'_> {
    fn append(&mut self, item: &dyn ToTokens) {
        item.to_tokens(self.output)
    }
}

trait Transcriber {
    fn transcribe(&self, ctx: &mut Ctx) -> syn::Result<()>;
}
impl Transcriber for ast::Transcribe {
    fn transcribe(&self, ctx: &mut Ctx) -> syn::Result<()> {
        match self {
            ast::Transcribe::Fragment(fragment) => unimplemented!(),
            ast::Transcribe::Repetition(repetition) => repetition.transcribe(ctx)?,
            ast::Transcribe::Group(group) => group.transcribe(ctx)?,
            ast::Transcribe::Ident(ident) => ctx.append(ident),
            ast::Transcribe::Literal(literal) => ctx.append(literal),
            ast::Transcribe::Punct(punct) => ctx.append(punct),
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

        ctx.append(&proc_macro2::Group::new(self.delimiter, output));
        Ok(())
    }
}
impl Transcriber for ast::TranscribeRepetition {
    fn transcribe(&self, ctx: &mut Ctx) -> syn::Result<()> {
        let fragments = find_level_fragments(self);

        unimplemented!();
    }
}

fn find_level_fragments(rep: &ast::TranscribeRepetition) -> Vec<&str> {
    let mut fragments = Set::default();
    fn find_inner<'a>(transcribe: &'a ast::Transcribe, fragments: &mut Set<&'a str>) {
        match transcribe {
            ast::Transcribe::Fragment(fragment) => {
                fragments.insert(&**fragment);
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
    let mut result = fragments.into_iter().collect::<Vec<&str>>();
    result.sort();
    result
}
