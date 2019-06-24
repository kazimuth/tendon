use proc_macro2 as pm2;
use quote::{quote, ToTokens};
use std::collections::HashMap;
use std::fmt::{Display, Write};
use syn::{self, ext::IdentExt, parse::ParseStream};

use crate::resolve::item_expand::ast;

enum Binding {
    Seq(Vec<Binding>),
    Leaf(pm2::TokenStream),
}
impl std::fmt::Debug for Binding {
    fn fmt(&self, w: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Binding::Seq(bs) => {
                write!(w, "[")?;
                for b in bs {
                    write!(w, "{:?}", b)?;
                }
                write!(w, "]")?;
            }
            Binding::Leaf(l) => {
                write!(w, "`{}`", l)?;
            }
        }
        Ok(())
    }
}
impl Binding {
    fn seq(&mut self) -> &mut Vec<Binding> {
        match self {
            Binding::Seq(v) => v,
            Binding::Leaf(_) => panic!("leaf at wrong level binding tree"),
        }
    }
    #[allow(unused)]
    fn seq_(&self) -> &Vec<Binding> {
        match self {
            Binding::Seq(v) => v,
            Binding::Leaf(_) => panic!("leaf at wrong level binding tree"),
        }
    }
}

#[derive(Debug)]
/// Tools used during consumption.
pub struct Stomach {
    /// Where we currently are within the stack of bindings.
    /// See the comment for the `item_expand` module.
    /// This is always rooted within a single frame with index 0.
    stack: Vec<usize>,

    /// Bound variables.
    bindings: HashMap<String, Binding>,

    /// Scratch; used for fast comparisons.
    scratch_a: String,
    /// Scratch; used for fast comparisons.
    scratch_b: String,

    /// Whether we're speculatively parsing tokens (during repetition separation)
    speculating: bool,
}

impl Stomach {
    pub fn new() -> Self {
        Stomach {
            stack: vec![0],
            bindings: HashMap::new(),
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
            |stream: ParseStream| -> syn::Result<()> { matchers.consume(self, stream) },
            input.clone(),
        )
    }

    fn debug(&mut self, _name: &str, _stream: ParseStream) {
        /*
        println!("=== {}", stream.cursor().token_stream());
        if self.speculating {
            print!("SPEC ");
        }
        println!("{}", name);
        */
    }

    /// Enter a repetition.
    fn enter_repetition<T, F: FnOnce(&mut Stomach) -> T>(&mut self, f: F) -> T {
        self.stack.push(0);
        let result = f(self);
        self.stack.pop();
        result
    }

    /// Move to the next group within a repetition.
    fn next_repetition(&mut self) {
        assert!(self.stack.len() > 1, "can't next root repetition");
        *self.stack.last_mut().unwrap() += 1;
    }

    fn is_first_repetition(&self) -> bool {
        *self.stack.last().expect("stack can't be empty") == 0
    }

    fn speculate<T, F: FnOnce(&mut Stomach) -> T>(&mut self, f: F) -> T {
        let prev = self.speculating;
        self.speculating = true;
        let result = f(self);
        self.speculating = prev;
        result
    }

    fn bind(&mut self, name: &pm2::Ident, value: pm2::TokenStream) {
        if self.speculating {
            return;
        }
        let Stomach {
            ref mut bindings,
            ref stack,
            ..
        } = *self;
        let name_ = format!("{}", name);
        let mut binding = bindings.entry(name_).or_insert_with(|| {
            let mut current = Binding::Seq(vec![]);
            for idx in stack[0..stack.len() - 1].iter().rev() {
                // if we're creating a new binding, we *must* be at position 0 along the whole stack
                // e.g. if we're matching $($($x:expr)), we gotta not have seen x before
                assert_eq!(*idx, 0, "binding that somehow wasn't bound earlier?");
                current = Binding::Seq(vec![current]);
            }
            current
        });

        for idx in &stack[0..stack.len() - 1] {
            binding = {
                let seq = binding.seq();
                if *idx == seq.len() {
                    seq.push(Binding::Seq(vec![]))
                }
                &mut seq[*idx]
            };
        }
        binding.seq().push(Binding::Leaf(value));
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
    fn consume(&self, inv: &mut Stomach, stream: ParseStream) -> syn::Result<()>;
    fn peek(&self, inv: &mut Stomach, stream: ParseStream) -> bool {
        self.consume(inv, &mut stream.fork()).is_ok()
    }
}
impl Consumer for ast::MatcherSeq {
    fn consume(&self, inv: &mut Stomach, stream: ParseStream) -> syn::Result<()> {
        inv.debug("MatcherSeq", stream);
        for matcher in &self.0 {
            let result = matcher.consume(inv, stream);
            result?;
        }
        Ok(())
    }
    fn peek(&self, inv: &mut Stomach, stream: ParseStream) -> bool {
        self.0[0].peek(inv, stream)
    }
}
impl Consumer for ast::Matcher {
    fn consume(&self, inv: &mut Stomach, stream: ParseStream) -> syn::Result<()> {
        inv.debug("Matcher", stream);
        let result = match self {
            ast::Matcher::Group(ref i) => i.consume(inv, stream),
            ast::Matcher::Ident(ref i) => i.consume(inv, stream),
            ast::Matcher::Literal(ref i) => i.consume(inv, stream),
            ast::Matcher::Punct(ref i) => i.consume(inv, stream),
            ast::Matcher::Fragment(ref i) => i.consume(inv, stream),
            ast::Matcher::Repetition(ref i) => i.consume(inv, stream),
        };
        result
    }
}
impl Consumer for ast::Fragment {
    fn consume(&self, inv: &mut Stomach, stream: ParseStream) -> syn::Result<()> {
        inv.debug("Fragment", stream);
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
        inv.bind(&self.ident, tokens);
        Ok(())
    }
}
impl Consumer for ast::Group {
    fn consume(&self, inv: &mut Stomach, stream: ParseStream) -> syn::Result<()> {
        inv.debug("Group", stream);
        let group = stream.parse::<pm2::Group>()?;
        if group.delimiter() != self.delimiter {
            return Err(syn::Error::new(
                group.span(),
                format!(
                    "wrong delimiters: expected {:?}, got {:?}",
                    self.delimiter,
                    group.delimiter()
                ),
            ));
        }
        let result = syn::parse::Parser::parse2(
            |stream: ParseStream| -> syn::Result<()> {
                self.inner.consume(inv, stream)?;
                Ok(())
            },
            group.stream(),
        );
        result
    }
    // fast peek: don't parse our insides
    fn peek(&self, _inv: &mut Stomach, stream: ParseStream) -> bool {
        match self.delimiter {
            pm2::Delimiter::Brace => stream.peek(syn::token::Brace),
            pm2::Delimiter::Parenthesis => stream.peek(syn::token::Paren),
            pm2::Delimiter::Bracket => stream.peek(syn::token::Bracket),
            _ => panic!("impossible"),
        }
    }
}
impl Consumer for ast::Repetition {
    fn consume(&self, inv: &mut Stomach, stream: ParseStream) -> syn::Result<()> {
        inv.debug("Repetition", stream);
        let result = inv.enter_repetition(|inv| {
            loop {
                let forked = stream.fork();
                let should_continue = inv.speculate(|inv| {
                    if inv.is_first_repetition() || self.sep.0.len() == 0 {
                        self.inner.peek(inv, &forked)
                    } else {
                        self.sep.peek(inv, &forked)
                    }
                });
                if !should_continue {
                    break;
                }
                if !inv.is_first_repetition() {
                    self.sep.consume(inv, stream)?;
                }
                self.inner.consume(inv, stream)?;
                inv.next_repetition();
            }
            Ok(())
        });
        result
    }
    fn peek(&self, inv: &mut Stomach, stream: ParseStream) -> bool {
        self.inner.peek(inv, stream)
    }
}
impl Consumer for ast::Sep {
    fn consume(&self, inv: &mut Stomach, stream: ParseStream) -> syn::Result<()> {
        inv.debug("Sep", stream);
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
    fn consume(&self, inv: &mut Stomach, stream: ParseStream) -> syn::Result<()> {
        inv.debug("Ident", stream);
        let actual = &stream.call(syn::Ident::parse_any)?;
        if self != actual {
            return Err(syn::Error::new(
                actual.span(),
                format!("pm2::Ident: expected {}, got {}", self, actual),
            ));
        }
        Ok(())
    }
}
impl Consumer for pm2::Literal {
    fn consume(&self, inv: &mut Stomach, stream: ParseStream) -> syn::Result<()> {
        inv.debug("Literal", stream);

        let actual = &stream.parse::<pm2::Literal>()?;
        if !inv.disp_eq(self, actual) {
            return Err(syn::Error::new(
                actual.span(),
                format!("pm2::Literal: expected {}, got {}", self, actual),
            ));
        }
        Ok(())
    }
}
impl Consumer for pm2::Punct {
    fn consume(&self, inv: &mut Stomach, stream: ParseStream) -> syn::Result<()> {
        inv.debug("Punct", stream);

        let actual = &stream.parse::<pm2::Punct>()?;
        // don't bother with spacing...
        if self.as_char() != actual.as_char() {
            return Err(syn::Error::new(
                actual.span(),
                format!("pm2::Punct: expected {}, got {}", self, actual),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_binding {
        ($inv:ident [$name:expr] $([$idx:expr])+ == $target:expr) => {
            match &$inv.bindings[$name] $(. seq_()[$idx])+ {
                Binding::Leaf(l) => assert_eq!(syn::parse2::<pm2::Ident>(l.clone())?, $target),
                _ => panic!("not a leaf, should be"),
            }
        }
    }

    #[test]
    fn full() -> syn::Result<()> {
        spoor::init();

        let matchers = syn::parse2::<ast::MatcherSeq>(quote! {
            $(pub fn $name:ident ($($arg:pat : $typ:ty),+) -> $ret:ty;)+
        })?;

        let mut inv = Stomach::new();
        let to_parse = quote! {
            pub fn squared(x: f32) -> f32;
            pub fn atan2(x: f32, y: f32) -> f32;
        };
        inv.consume(&to_parse, &matchers)?;

        assert_binding!(inv["name"][0][0] == "squared");
        assert_binding!(inv["arg"][0][0][0] == "x");
        assert_binding!(inv["typ"][0][0][0] == "f32");
        assert_binding!(inv["ret"][0][0] == "f32");

        assert_binding!(inv["name"][0][1] == "atan2");
        assert_binding!(inv["arg"][0][1][0] == "x");
        assert_binding!(inv["typ"][0][1][0] == "f32");
        assert_binding!(inv["arg"][0][1][1] == "y");
        assert_binding!(inv["typ"][0][1][1] == "f32");
        assert_binding!(inv["ret"][0][1] == "f32");

        Ok(())
    }

    #[test]
    fn simple_repetition() -> syn::Result<()> {
        spoor::init();

        let matchers = syn::parse2::<ast::MatcherSeq>(quote! { $(bees)+ })?;
        let to_parse = &quote! { bees bees bees bees bees };
        let mut inv = Stomach::new();
        inv.consume(to_parse, &matchers)?;

        Ok(())
    }

    #[test]
    fn recursive_repetition() -> syn::Result<()> {
        spoor::init();

        let matchers = syn::parse2::<ast::MatcherSeq>(quote! { $(($($name:ident)+))+ })?;
        let to_parse = &quote! { (jane ben harper) (xanadu xylophone)};

        let mut inv = Stomach::new();
        inv.consume(to_parse, &matchers)?;

        Ok(())
    }

    #[test]
    fn nonterminal_fragments() -> syn::Result<()> {
        spoor::init();

        let matchers = syn::parse2::<ast::MatcherSeq>(quote! { $x:expr })?;
        let to_parse = &quote! { 1 + 1 * (37 + _umlaut[&|| {}]) };

        let mut inv = Stomach::new();
        inv.consume(to_parse, &matchers)?;

        Ok(())
    }

    /*
    #[test]
    fn macro_expansion() {
        macro_rules! t {
            ($($outer:ident [$($inner:ident)+])* ( $($o2:ident [$($i2:ident)*] )* )) => {
                stringify!($($outer $o2 [$($inner $i2)+])+)
            }
        }
        println!("{}", t!(a [b c d] f [g h] (a2 [b2 c2 d2] f2 [g2 h2] )));
        macro_rules! v {
            ($(($($x:ident)*))*) => {
                stringify!($($($x)+)+)
            }
        }
        println!("{}", v!((a b c) (d f)));
    }
    */
}
