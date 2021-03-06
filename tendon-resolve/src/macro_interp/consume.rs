//! Algorithm to consume a macro input stream, saving matched fragments to `Binding`s as we go.
//!
//! Based heavily on libsyntax_ext's
//! [macro transcription code](https://github.com/rust-lang/rust/blob/12806b7/src/libsyntax/ext/tt/transcribe.rs).

use proc_macro2 as pm2;
use quote::ToTokens;
use std::fmt::{Display, Write};
use syn::{self, ext::IdentExt, parse::ParseStream};
use tendon_api::Map;

use crate::macro_interp::ast;
use crate::macro_interp::ast::RepeatKind;

/// A fragment binding.
///
/// Every fragment in a matcher is mapped to a tree of bindings.
///
/// For example, if we have:
///
/// `$({$($value:expr),+})+`
///
/// We can match:
/// `{1,2,3} {4,5} {6,7,8,9}`
///
/// Which will set `$value`'s `Binding` to:
///
/// ```no_build
/// [
///     [
///         [`1`, `2`, `3`],
///         [`4`, `5`],
///         [`6`, `7`, `8`, `9`],
///     ]
/// ]
/// ```
pub enum Binding {
    Seq(Vec<Binding>),
    Leaf(pm2::TokenStream),
}
impl std::fmt::Debug for Binding {
    fn fmt(&self, w: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Binding::Seq(bs) => {
                write!(w, "[")?;
                let mut first = true;
                for b in bs {
                    if first {
                        first = false;
                    } else {
                        write!(w, ",")?;
                    }
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
    /// Insert a binding.
    /// Inserts Binding::Seqs as it goes as needed to get to the requested location.
    /// returns whether the insert was successful.
    pub fn insert(&mut self, indices: &[usize], tokens: pm2::TokenStream) -> bool {
        if indices.len() == 0 {
            *self = Binding::Leaf(tokens);
            return true;
        }
        let mut result = self;
        for index in &indices[0..indices.len() - 1] {
            if let Binding::Seq(seq) = result {
                // necessary if we're looking up a slot for a binding that hasn't been bound yet.
                while seq.len() <= *index {
                    seq.push(Binding::Seq(vec![]))
                }

                result = &mut seq[*index];
            } else {
                return false;
            }
        }

        if let Binding::Seq(seq) = result {
            let index = indices[indices.len() - 1];
            if seq.len() == index {
                seq.push(Binding::Leaf(tokens));
                return true;
            }
        }
        false
    }
    /// Same as get_mut but doesn't ever modify the input binding.
    pub fn get(&self, indices: &[usize]) -> Option<&Binding> {
        // this is slightly hacky but it works...
        let mut result = self;
        for index in indices {
            if let Binding::Seq(seq) = result {
                result = seq.get(*index)?;
            } else {
                return None;
            }
        }
        Some(result)
    }
}

#[derive(Debug)]
/// Tools used during macro consumption consumption.
pub struct Stomach {
    /// Where we currently are within the stack of bindings.
    /// This is always rooted within a single frame with index 0.
    stack: Vec<usize>,

    /// Bound variables.
    pub bindings: Map<String, Binding>,

    /// Scratch; used for fast comparisons.
    scratch_a: String,
    /// Scratch; used for fast comparisons.
    scratch_b: String,

    /// Whether we're speculatively parsing tokens (during repetition separation)
    speculating: bool,
}

impl Stomach {
    /// Create a new Stomach.
    pub fn new() -> Self {
        Stomach {
            stack: vec![],
            bindings: Map::default(),
            scratch_a: String::new(),
            scratch_b: String::new(),
            speculating: false,
        }
    }

    /// Reset all internal state.
    pub fn reset(&mut self) {
        self.stack = vec![];
        self.bindings.clear();
        self.scratch_a.clear();
        self.scratch_b.clear();
        self.speculating = false;
    }

    /// Consume an input token stream.
    ///
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
        //trace!("=== {}", stream.cursor().token_stream());
        //if self.speculating {
        //    trace!("SPEC ");
        //}
        //trace!("{}", name);
    }

    /// Enter a repetition.
    fn enter_repetition<T, F: FnOnce(&mut Stomach) -> T>(&mut self, f: F) -> T {
        self.stack.push(0);
        let result = f(self);
        self.stack.pop();
        result
    }

    /// Move to the next group within a repetition.
    fn next_repetition(&mut self, stream: ParseStream) -> syn::Result<()> {
        if self.stack.len() == 0 {
            return Err(stream.error("No next repetition for root binding"));
        }
        *self.stack.last_mut().unwrap() += 1;
        Ok(())
    }

    /// If we are within the first repetition of a sequence of repetitions.
    fn is_first_repetition(&self) -> bool {
        *self.stack.last().expect("stack can't be empty") == 0
    }

    /// Set our mode to speculatively parsing (for figuring out if we should exit a repetition).
    fn speculate<T, F: FnOnce(&mut Stomach) -> T>(&mut self, f: F) -> T {
        let prev = self.speculating;
        self.speculating = true;
        let result = f(self);
        self.speculating = prev;
        result
    }

    /// non-allocating comparison for two types that only impl Display, like Literal in syn
    fn disp_eq(&mut self, a: &impl Display, b: &impl Display) -> bool {
        // can't fail
        let _ = write!(&mut self.scratch_a, "{}", a);
        let _ = write!(&mut self.scratch_b, "{}", b);
        let result = self.scratch_a == self.scratch_b;
        self.scratch_a.clear();
        self.scratch_b.clear();
        result
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
            ast::FragSpec::Expr => stream.parse::<syn::Expr>()?.into_token_stream(),
            ast::FragSpec::Literal => stream.parse::<syn::Lit>()?.into_token_stream(),
            ast::FragSpec::Statement => stream.parse::<syn::Stmt>()?.into_token_stream(),
            ast::FragSpec::Block => stream.parse::<syn::Block>()?.into_token_stream(),
        };

        if inv.speculating {
            return Ok(());
        }

        // bind this fragment

        let Stomach {
            ref mut bindings,
            ref stack,
            ..
        } = *inv;

        let name = format!("{}", self.ident);

        // Look up the binding for this fragment.
        // If it doesn't exist, we create one, consisting of nested empty seqs to the current stack level.
        let binding = bindings.entry(name).or_insert_with(|| Binding::Seq(vec![]));

        if binding.insert(&stack[..], tokens) {
            Ok(())
        } else {
            Err(stream.error(format!(
                "failed to bind ${}:{:?} : token matched at earlier level",
                self.ident, self.spec
            )))
        }
    }
}
impl Consumer for ast::Group {
    fn consume(&self, inv: &mut Stomach, stream: ParseStream) -> syn::Result<()> {
        inv.debug("Group", stream);
        let group = stream.parse::<pm2::Group>()?;
        if group.delimiter() != self.delimiter {
            return Err(stream.error(format!(
                "wrong delimiters: expected {:?}, got {:?}",
                self.delimiter,
                group.delimiter()
            )));
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
            _ => unreachable!(),
        }
    }
}
impl Consumer for ast::Repetition {
    fn consume(&self, inv: &mut Stomach, stream: ParseStream) -> syn::Result<()> {
        inv.debug("Repetition", stream);
        let mut count = 0;
        let result = inv.enter_repetition(|inv| {
            loop {
                if self.kind == RepeatKind::Question && count == 1 {
                    break;
                }
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
                inv.next_repetition(stream)?;
                count += 1
            }
            Ok(())
        });
        if self.kind == RepeatKind::Plus && count == 0 {
            return Err(stream.error("not enough repetitions of kleene plus"));
        }
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

        let actual = stream.parse::<pm2::TokenTree>()?;

        let actual = if let pm2::TokenTree::Punct(p) = actual {
            p
        } else {
            return Err(syn::Error::new(
                actual.span(),
                format!("pm2::Punct: expected {}, got {}", self, actual),
            ));
        };

        //let actual = &stream.step(|cursor| {
        //    cursor.punct().ok_or_else(|| cursor.error("expected punct"))
        //})?;

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
    use quote::quote;

    fn consume(
        matcher: pm2::TokenStream,
        input: pm2::TokenStream,
    ) -> Result<Map<String, Binding>, syn::Error> {
        let matchers = syn::parse2::<ast::MatcherSeq>(matcher)?;
        let mut stomach = Stomach::new();
        stomach.consume(&input, &matchers)?;
        Ok(stomach.bindings)
    }

    #[test]
    fn full() -> syn::Result<()> {
        spoor::init();

        let bindings = consume(
            quote! { $(pub fn $name:ident ($($arg:pat : $typ:ty),+) -> $ret:ty;)+ },
            quote! {
                pub fn squared(x: f32) -> f32;
                pub fn atan2(x: f32, y: f32) -> f32;
            },
        )?;

        macro_rules! assert_binding {
            ($bindings:ident [$name:expr, $($idx:expr),+] == $target:expr) => {

                match &$bindings[$name].get(&[$($idx),+]) {
                    Some(Binding::Leaf(l)) => assert_eq!(syn::parse2::<pm2::Ident>(l.clone())?, $target),
                    _ => panic!("not a leaf, should be"),
                }
            }
        }

        assert_binding!(bindings["name", 0] == "squared");
        assert_binding!(bindings["arg", 0, 0] == "x");
        assert_binding!(bindings["typ", 0, 0] == "f32");
        assert_binding!(bindings["ret", 0] == "f32");

        assert_binding!(bindings["name", 1] == "atan2");
        assert_binding!(bindings["arg", 1, 0] == "x");
        assert_binding!(bindings["typ", 1, 0] == "f32");
        assert_binding!(bindings["arg", 1, 1] == "y");
        assert_binding!(bindings["typ", 1, 1] == "f32");
        assert_binding!(bindings["ret", 1] == "f32");

        Ok(())
    }

    #[test]
    fn repetition() -> syn::Result<()> {
        spoor::init();

        // simple
        consume(quote! { $(bees)+ }, quote! { bees bees bees bees bees })?;
        // recursive
        consume(
            quote! { $(($($name:ident)+))+ },
            quote! { (jane ben harper) (xanadu xylophone)},
        )?;
        // weird sep (note: this is valid rust code!)
        consume(quote! { $(_)bees+ }, quote! { _ bees _ bees _ bees _ })?;
        // group sep (forbidden)
        assert_match!(consume(quote! { $(_)[]* }, quote! {}), Err(..));
        // kleene+
        assert_match!(consume(quote! { hello $(bees)+ }, quote! {hello}), Err(..));

        Ok(())
    }

    #[test]
    fn mismatches() -> syn::Result<()> {
        spoor::init();

        assert_match!(consume(quote! { (bees) }, quote! { {bees} }), Err(..));
        assert_match!(consume(quote! { bees }, quote! { wasps }), Err(..));
        assert_match!(consume(quote! { ! }, quote! { ? }), Err(..));

        Ok(())
    }

    #[test]
    fn non_terminal_fragments() -> syn::Result<()> {
        spoor::init();

        let bindings = consume(
            quote! { $x:expr },
            quote! { 1 + 1 * (37 + _umlaut[&|| {}]) },
        )?;

        assert_eq!(
            &format!("{:?}", bindings["x"]),
            "`1 + 1 * ( 37 + _umlaut [ & | | { } ] )`"
        );

        Ok(())
    }

    #[test]
    fn match_literal() -> syn::Result<()> {
        spoor::init();

        assert_match!(consume(quote!("hello"), quote!("hello")), Ok(..));
        assert_match!(consume(quote!("hello"), quote!("goodbye")), Err(..));

        Ok(())
    }

    #[test]
    fn all_fragment_specifiers() -> syn::Result<()> {
        spoor::init();

        consume(
            quote!($thing:block),
            quote!({
                return;
            }),
        )?;

        consume(quote!($thing:expr), quote!({ 1 + "hello" }))?;
        consume(quote!($thing:ident), quote!(zanzibar))?;
        consume(
            quote!($thing:item),
            quote!(
                type X<T> = B;
            ),
        )?;
        consume(quote!($thing:lifetime), quote!('short))?;

        consume(quote!($thing:literal), quote!(3.14159f64))?;
        consume(quote!($thing:meta), quote!(frag))?;
        consume(quote!($thing:pat), quote!(Banana(ocelot, ..)))?;
        consume(quote!($thing:path), quote!(::f::x<i32>::y<'a>))?;
        consume(quote!($thing:stmt), quote!(break;))?;
        consume(quote!($thing:tt), quote!({ banana }))?;
        consume(
            quote!($thing:ty),
            quote!(&[impl Banana<'a, f32> + Copy + ?Sized]),
        )?;
        consume(quote!($thing:vis), quote!(pub(crate)))?;

        Ok(())
    }
}
