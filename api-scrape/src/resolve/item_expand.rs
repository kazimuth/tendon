//! Simplified macro expansion for items. Does not handle expressions at all (expands them to ()),
//! since we only need the result for its signature.
//! Implemented as an interpreter on top of syn.

// from rust reference, https://doc.rust-lang.org/stable/reference/macros-by-example.html:

// > When forwarding a matched fragment to another macro-by-example, matchers in the second macro will
// > see an opaque AST of the fragment type. The second macro can't use literal tokens to match the fragments
// > in the matcher, only a fragment specifier of the same type. The ident, lifetime, and tt fragment types
// > are an exception, and can be matched by literal tokens.

//> The specific rules are:
//> expr and stmt may only be followed by one of: =>, ,, or ;.
//> pat may only be followed by one of: =>, ,, =, ,, if, or in.
//> path and ty may only be followed by one of: =>, ,, =, ,, ;, :, >, >>, [, {, as, where, or
//>     a macro variable of block fragment specifier.
//> vis may only be followed by one of: ,, an identifier other than a non-raw priv, any token
//>     that can begin a type, or a metavariable with a ident, ty, or path fragment specifier.
//> All other fragment specifiers have no restrictions.

// TODO: set macro recursion depth high
// TODO: multiple matchers per level?
// TODO: repro weird trace span-drops

use proc_macro2 as pm2;
use quote::{quote, ToTokens};
use std::collections::HashMap;
use std::fmt::{Display, Write};
use syn::{self, parse::ParseStream};
use tokio_trace::{trace, trace_span, warn};

pub mod ast;

// TODO repetitions?

#[derive(Default, Debug)]
struct RecursionLevel {
    bindings: HashMap<pm2::Ident, Vec<pm2::TokenStream>>,
}
/// A single macro invocation.
pub struct Invocation {
    levels: Vec<RecursionLevel>,
    current_level: usize,
    macro_name: pm2::Ident,
    // used for not allocating during `display` comparisons
    scratch_a: String,
    scratch_b: String,
    // whether we're speculatively parsing tokens (during repetition separation)
    speculating: bool,
}
impl Invocation {
    pub fn new(macro_name: pm2::Ident) -> Self {
        Invocation {
            levels: vec![RecursionLevel {
                bindings: HashMap::new(),
            }],
            current_level: 0,
            macro_name,
            scratch_a: String::new(),
            scratch_b: String::new(),
            speculating: false,
        }
    }
    pub fn consume(
        &mut self,
        input: &pm2::TokenStream,
        matchers: &ast::MatcherSeq,
    ) -> syn::Result<()> {
        // kinda a hack to convert a tokenstream to a parsestream... whatever
        syn::parse::Parser::parse2(
            |stream: ParseStream| -> syn::Result<()> {
                matchers.consume(self, stream)?;
                Ok(())
            },
            input.clone(),
        )
    }

    /// Raise the recursion level within a closure.
    fn raise_level<T, F: FnOnce(&mut Invocation) -> T>(&mut self, f: F) -> T {
        self.current_level += 1;
        if self.current_level == self.levels.len() {
            self.levels.push(RecursionLevel {
                bindings: HashMap::new(),
            })
        }
        let result = f(self);
        self.current_level -= 1;
        result
    }

    fn speculate<T, F: FnOnce(&mut Invocation) -> T>(&mut self, f: F) -> T {
        let span = trace_span!("SPEC");
        let entered = span.enter();
        trace!("--->");

        let prev = self.speculating;
        self.speculating = true;
        let result = f(self);
        drop(entered);
        trace!("<---");
        self.speculating = prev;
        result
    }

    fn bind(&mut self, name: &pm2::Ident, value: pm2::TokenStream) {
        if self.speculating {
            return;
        }

        let bindings = self.levels[self.current_level]
            .bindings
            .entry(name.clone())
            .or_insert_with(|| vec![]);
        bindings.push(value);
        if self.current_level == 0 && bindings.len() > 1 {
            warn!(
                "multiple bindings for {} in {}! at level 0",
                name, self.macro_name
            )
        }
    }

    /// non-allocating comparison for two types that only impl Display, like Literal in syn
    fn disp_eq(&mut self, a: &impl Display, b: &impl Display) -> bool {
        self.scratch_a.clear();
        self.scratch_b.clear();
        // can't fail
        let _ = write!(&mut self.scratch_a, "{}", a);
        let _ = write!(&mut self.scratch_b, "{}", b);
        self.scratch_a == self.scratch_b
    }
}
trait Consumer {
    fn consume(&self, inv: &mut Invocation, stream: ParseStream) -> syn::Result<()>;
    fn peek(&self, inv: &mut Invocation, stream: ParseStream) -> bool {
        self.consume(inv, &mut stream.fork()).is_ok()
    }
}
trait Producer {
    fn produce(&self, inv: &mut Invocation) -> syn::Result<pm2::TokenStream>;
}
impl Consumer for ast::MatcherSeq {
    fn consume(&self, inv: &mut Invocation, stream: ParseStream) -> syn::Result<()> {
        let span = trace_span!("MS");
        let entered = span.enter();
        trace!(">");

        for (i, matcher) in self.0.iter().enumerate() {
            trace!("{}", i);
            matcher.consume(inv, stream)?;
        }
        if !stream.is_empty() {
            let item = stream.parse::<pm2::TokenTree>()?;
            Err(syn::Error::new(
                item.span(),
                format!("ast::MatcherSeq: unexpected token, should be EOS: {}", item),
            ))?;
        }
        drop(entered);
        trace!("<");
        Ok(())
    }
    fn peek(&self, inv: &mut Invocation, stream: ParseStream) -> bool {
        self.0[0].peek(inv, stream)
    }
}
impl Consumer for ast::Matcher {
    fn consume(&self, inv: &mut Invocation, stream: ParseStream) -> syn::Result<()> {
        //let span = trace_span!("Matcher");
        //let _entered = span.enter();
        //trace!(">");

        let result = match self {
            ast::Matcher::Group(ref i) => i.consume(inv, stream),
            ast::Matcher::Ident(ref i) => i.consume(inv, stream),
            ast::Matcher::Literal(ref i) => i.consume(inv, stream),
            ast::Matcher::Punct(ref i) => i.consume(inv, stream),
            ast::Matcher::Fragment(ref i) => i.consume(inv, stream),
            ast::Matcher::Repetition(ref i) => i.consume(inv, stream),
        };
        //trace!("<");
        result
    }
}
impl Consumer for ast::Fragment {
    fn consume(&self, inv: &mut Invocation, stream: ParseStream) -> syn::Result<()> {
        let tokens = match self.spec {
            ast::FragSpec::Ident => stream.parse::<syn::Ident>()?.into_token_stream(),
            ast::FragSpec::Item => stream.parse::<syn::Item>()?.into_token_stream(),
            ast::FragSpec::Lifetime => stream.parse::<syn::Lifetime>()?.into_token_stream(),
            ast::FragSpec::Meta => stream.parse::<syn::Meta>()?.into_token_stream(),
            ast::FragSpec::Pattern => stream.parse::<syn::Pat>()?.into_token_stream(),
            ast::FragSpec::Path => stream.parse::<syn::Path>()?.into_token_stream(),
            ast::FragSpec::TokenTree => stream.parse::<pm2::TokenTree>()?.into_token_stream(),
            ast::FragSpec::Type => stream.parse::<syn::Type>()?.into_token_stream(),
            ast::FragSpec::Visibility => stream.parse::<syn::Visibility>()?.into_token_stream(),

            // note: unneeded for item parsing; we throw these out 'cause there's no hygiene anyway
            ast::FragSpec::Expr => {
                stream.parse::<syn::Expr>()?;
                quote!(_)
            }
            ast::FragSpec::Literal => {
                stream.parse::<syn::Lit>()?;
                quote!(_)
            }
            ast::FragSpec::Statement => {
                stream.parse::<syn::Stmt>()?;
                quote!({ _ })
            }
            ast::FragSpec::Block => {
                stream.parse::<syn::Block>()?;
                quote!({ _ })
            }
        };
        trace!("Fragment ${}: `{}`", self.ident, tokens);
        inv.bind(&self.ident, tokens);
        Ok(())
    }
}
impl Consumer for ast::Group {
    fn consume(&self, inv: &mut Invocation, stream: ParseStream) -> syn::Result<()> {
        let span = trace_span!("GRP");
        let entered = span.enter();
        trace!(">");

        let group = stream.parse::<pm2::Group>()?;
        if group.delimiter() != self.delimiter {
            Err(syn::Error::new(
                group.span(),
                format!(
                    "wrong delimiters: expected {:?}, got {:?}",
                    self.delimiter,
                    group.delimiter()
                ),
            ))?;
        }
        let result = syn::parse::Parser::parse2(
            |stream: ParseStream| -> syn::Result<()> {
                self.inner.consume(inv, stream)?;
                Ok(())
            },
            group.stream(),
        );
        drop(entered);
        trace!("<");
        result
    }
    // fast peek: don't parse our insides
    fn peek(&self, _inv: &mut Invocation, stream: ParseStream) -> bool {
        match self.delimiter {
            pm2::Delimiter::Brace => stream.peek(syn::token::Brace),
            pm2::Delimiter::Parenthesis => stream.peek(syn::token::Paren),
            pm2::Delimiter::Bracket => stream.peek(syn::token::Bracket),
            _ => panic!("impossible"),
        }
    }
}
impl Consumer for ast::Repetition {
    fn consume(&self, inv: &mut Invocation, stream: ParseStream) -> syn::Result<()> {
        let span = trace_span!("REP");
        let entered = span.enter();
        trace!(">");

        let result = inv.raise_level(|inv| {
            let mut first = true;
            loop {
                let forked = stream.fork();
                let should_continue = inv.speculate(|inv| {
                    if first || self.sep.0.len() == 0 {
                        first = false;
                        self.inner.peek(inv, &forked)
                    } else {
                        self.sep.peek(inv, &forked)
                    }
                });
                if !should_continue {
                    break;
                }
                self.inner.consume(inv, stream)?;
            }
            Ok(())
        });

        drop(entered);
        trace!("<");
        result
    }
    fn peek(&self, inv: &mut Invocation, stream: ParseStream) -> bool {
        self.inner.peek(inv, stream)
    }
}
impl Consumer for ast::Sep {
    fn consume(&self, inv: &mut Invocation, stream: ParseStream) -> syn::Result<()> {
        trace!("Sep");

        for c in &self.0 {
            match c {
                pm2::TokenTree::Ident(correct) => correct.consume(inv, stream)?,
                pm2::TokenTree::Literal(correct) => correct.consume(inv, stream)?,
                pm2::TokenTree::Punct(correct) => correct.consume(inv, stream)?,
                pm2::TokenTree::Group(g) => Err(syn::Error::new(
                    g.span(),
                    "ast::Sep: can't have a group in a sep",
                ))?,
            }
        }
        Ok(())
    }
}
impl Consumer for pm2::Ident {
    fn consume(&self, _inv: &mut Invocation, stream: ParseStream) -> syn::Result<()> {
        let actual = &stream.parse::<pm2::Ident>()?;
        if self != actual {
            Err(syn::Error::new(
                actual.span(),
                format!("pm2::Ident: expected {}, got {}", self, actual),
            ))?;
        }

        trace!("Ident `{}`", self);
        Ok(())
    }
}
impl Consumer for pm2::Literal {
    fn consume(&self, inv: &mut Invocation, stream: ParseStream) -> syn::Result<()> {
        trace!("Literal");

        let actual = &stream.parse::<pm2::Literal>()?;
        if !inv.disp_eq(self, actual) {
            Err(syn::Error::new(
                actual.span(),
                format!("pm2::Literal: expected {}, got {}", self, actual),
            ))?;
        }

        Ok(())
    }
}
impl Consumer for pm2::Punct {
    fn consume(&self, _inv: &mut Invocation, stream: ParseStream) -> syn::Result<()> {
        let actual = &stream.parse::<pm2::Punct>()?;
        // don't bother with spacing...
        if self.as_char() != actual.as_char() {
            Err(syn::Error::new(
                actual.span(),
                format!("pm2::Punct: expected {}, got {}", self, actual),
            ))?;
        }

        trace!("Punct `{}`", self.as_char());
        Ok(())
    }
}

impl Producer for ast::TranscriberSeq {
    fn produce(&self, _inv: &mut Invocation) -> syn::Result<pm2::TokenStream> {
        unimplemented!()
    }
}
impl Producer for ast::Transcriber {
    fn produce(&self, _inv: &mut Invocation) -> syn::Result<pm2::TokenStream> {
        unimplemented!()
    }
}
impl Producer for ast::TranscribeGroup {
    fn produce(&self, _inv: &mut Invocation) -> syn::Result<pm2::TokenStream> {
        unimplemented!()
    }
}
impl Producer for ast::TranscribeRepetition {
    fn produce(&self, _inv: &mut Invocation) -> syn::Result<pm2::TokenStream> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn consume() -> syn::Result<()> {
        spoor::init();

        let matchers =
            syn::parse_str::<ast::MatcherSeq>("$(($input:expr) $binding:pat => $then:expr),+")?;

        let mut inv = Invocation::new(parse_quote! { test_macro });
        let to_parse = &quote! {
            (seq.0[0]) Matcher::Ident(ident) => assert_eq!(ident, "ocelot")
        };
        inv.consume(to_parse, &matchers)?;
        assert_eq!(inv.levels[1].bindings[&parse_quote! { input }].len(), 2);
        assert_eq!(inv.levels[1].bindings[&parse_quote! { binding }].len(), 2);
        assert_eq!(inv.levels[1].bindings[&parse_quote! { then }].len(), 2);

        Ok(())
    }
}
