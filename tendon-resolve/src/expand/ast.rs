//! A syn parser for `macro_rules!`.
//!
//!>    MacroRulesDefinition :
//!>       macro_rules ! IDENTIFIER MacroRulesDef
//!>    MacroRulesDef :
//!>          ( MacroRules ) ;
//!>       , \[ MacroRules \] ;
//!>       , { MacroRules }
//!>    MacroRules :
//!>       MacroRule ( ; MacroRule )\* ;\?
//!>    MacroRule :
//!>       MacroMatcher => MacroTranscriber
//!>    MacroMatcher :
//!>          ( MacroMatch\* )
//!>       , [ MacroMatch\* ]
//!>       , { MacroMatch\* }
//!>    MacroMatch :
//!>          Token[except $ and delimiters]
//!>       , MacroMatcher
//!>       , $ IDENTIFIER : MacroFragSpec
//!>       , $ ( MacroMatch\+ ) MacroRepSep\? MacroRepOp
//!>    MacroFragSpec :
//!>          block , expr , ident , item , lifetime , literal
//!>       , meta , pat , path , stmt , tt , ty , vis
//!>    MacroRepSep :
//!>       Tokenexcept delimiters and repetition operators
//!>    MacroRepOp[2018+] :
//!>       * , + , ?[2018+]
//!>    MacroTranscriber :
//!>       DelimTokenTree

use proc_macro2 as pm2;
use syn::{
    self, parenthesized,
    parse::{Parse, ParseStream},
    spanned::Spanned,
    token, Token,
};

/// A full `macro_rules!` definition.
#[derive(Debug)]
pub struct MacroDef {
    pub attrs: Vec<syn::Attribute>,
    pub ident: pm2::Ident,
    pub rules: Vec<MacroRule>,
}

/// An individual macro_rule, consisting of a matcher and a transcriber.
#[derive(Debug)]
pub struct MacroRule {
    pub matcher: MatcherSeq,
    pub transcriber: TranscribeSeq,
}

/// A sequence of matchers.
#[derive(Debug)]
pub struct MatcherSeq(pub Vec<Matcher>);

/// A sequence of transcribers.
#[derive(Debug)]
pub struct TranscribeSeq(pub Vec<Transcribe>);

/// All of the possible elements that can be matched in a macro.
#[derive(Debug)]
pub enum Matcher {
    Repetition(Repetition),
    Fragment(Fragment),
    Group(Group),
    Ident(pm2::Ident),
    Literal(pm2::Literal),
    Punct(pm2::Punct),
}

/// A macro repetition `$(...),+`.
#[derive(Debug)]
pub struct Repetition {
    pub inner: MatcherSeq,
    pub sep: Sep,
    pub kind: RepeatKind,
}

/// Kind of macro repetition: `+`, `*`, or `?`.
#[derive(Debug)]
pub enum RepeatKind {
    Plus,
    Star,
    Question,
}

/// A macro repetition separator.
/// Strictly speaking this can be any individual rust token, but there's
/// no easy way to represent that with syn / pm2, so we just have a vec of
/// pm2::TokenTrees.
#[derive(Debug)]
pub struct Sep(pub Vec<pm2::TokenTree>);

/// A binding fragment: `$x:ident`, `$type:ty`, `$next:tt`, etc.
#[derive(Debug)]
pub struct Fragment {
    pub ident: String,
    pub spec: FragSpec,
}
/// A fragment specifier: `expr`, `stmt`, `block`, `tt`, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragSpec {
    Block,
    Expr,
    Ident,
    Item,
    Lifetime,
    Literal,
    Meta,
    Pattern,
    Path,
    Statement,
    TokenTree,
    Type,
    Visibility,
}

/// A group delimited by some delimiter: (...), {...}, [...].
/// Note: NOT a `Repetition`!
#[derive(Debug)]
pub struct Group {
    pub delimiter: pm2::Delimiter,
    pub inner: MatcherSeq,
}

/// Everything that a macro can transcribe.
#[derive(Debug)]
pub enum Transcribe {
    // TODO: can be a false match?
    Fragment(TranscribeFragment),
    Repetition(TranscribeRepetition),
    Group(TranscribeGroup),
    Ident(pm2::Ident),
    Literal(pm2::Literal),
    Punct(pm2::Punct),
}
/// A repeated transcription, $(...)+.
#[derive(Debug)]
pub struct TranscribeRepetition {
    pub sep: Sep,
    pub inner: TranscribeSeq,
}

/// A transcription of a delimited token tree, `(...)`, `[...]`, `{...}`.
#[derive(Debug)]
pub struct TranscribeGroup {
    pub delimiter: pm2::Delimiter,
    pub inner: TranscribeSeq,
}

#[derive(Debug)]
/// A fragment transcription, `$thing`
pub struct TranscribeFragment(pub String);

impl Parse for MacroDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let syn::ItemMacro {
            ident, mac, attrs, ..
        } = syn::ItemMacro::parse(input)?;
        if !mac.path.is_ident("macro_rules") {
            return Err(syn::Error::new(mac.span(), "not macro_rules"));
        }
        let ident = ident.ok_or(syn::Error::new(
            mac.span(),
            "no macro_ident in macro_rules!",
        ))?;

        let rules = syn::parse2::<MacroRules>(mac.tokens)?.0;

        Ok(MacroDef {
            ident,
            attrs,
            rules,
        })
    }
}
struct MacroRules(Vec<MacroRule>);
impl Parse for MacroRules {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut result = MacroRules(vec![]);
        while !input.is_empty() {
            result.0.push(input.parse::<MacroRule>()?);
        }
        Ok(result)
    }
}

impl Parse for MacroRule {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let matcher = input.parse::<pm2::Group>()?;
        let matcher = syn::parse2::<MatcherSeq>(matcher.stream())?;
        input.parse::<Token![=>]>()?;

        let transcriber = input.parse::<pm2::Group>()?;
        let transcriber = syn::parse2::<TranscribeSeq>(transcriber.stream())?;

        if input.lookahead1().peek(Token![;]) {
            input.parse::<Token![;]>()?;
        }

        Ok(MacroRule {
            matcher,
            transcriber,
        })
    }
}
impl Parse for MatcherSeq {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut result = MatcherSeq(vec![]);
        while !input.is_empty() {
            result.0.push(input.parse::<Matcher>()?);
        }
        Ok(result)
    }
}
impl Parse for Matcher {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(token::Dollar) {
            if input.peek2(token::Paren) {
                Ok(Matcher::Repetition(input.parse::<Repetition>()?))
            } else {
                Ok(Matcher::Fragment(input.parse::<Fragment>()?))
            }
        } else {
            let tt = input.parse::<pm2::TokenTree>()?;
            match tt {
                pm2::TokenTree::Ident(ident) => Ok(Matcher::Ident(ident)),
                pm2::TokenTree::Literal(literal) => Ok(Matcher::Literal(literal)),
                pm2::TokenTree::Punct(punct) => Ok(Matcher::Punct(punct)),
                pm2::TokenTree::Group(group) => Ok(Matcher::Group(Group {
                    delimiter: group.delimiter(),
                    inner: syn::parse2::<MatcherSeq>(group.stream())?,
                })),
            }
        }
    }
}
impl Parse for Repetition {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<token::Dollar>()?; // $
        let inner;
        parenthesized!(inner in input);
        let inner = inner.parse::<MatcherSeq>()?;
        let sep = input.parse::<Sep>()?;
        let lookahead = input.lookahead1();
        let kind = if lookahead.peek(Token![?]) {
            input.parse::<Token![?]>()?;
            RepeatKind::Question
        } else if lookahead.peek(Token![*]) {
            input.parse::<Token![*]>()?;
            RepeatKind::Star
        } else if lookahead.peek(Token![+]) {
            input.parse::<Token![+]>()?;
            RepeatKind::Plus
        } else {
            return Err(lookahead.error());
        };

        Ok(Repetition { inner, sep, kind })
    }
}
impl Parse for Sep {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // there's no easy way to parse "one token" (pm2 is too low-level)
        // so we just accept more than we should; rustc should already have weeded out incorrect seps
        let mut sep = vec![];
        while !input.peek(Token![*]) && !input.peek(Token![+]) && !input.peek(Token![?]) {
            let tt = input.parse::<pm2::TokenTree>()?;
            if let pm2::TokenTree::Group(ref group) = tt {
                Err(syn::Error::new(group.span(), "group in repetition sep???"))?;
            }
            sep.push(tt);
        }

        Ok(Sep(sep))
    }
}
impl Parse for Fragment {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<token::Dollar>()?; // $
        let ident = input.parse::<pm2::Ident>()?;
        let ident = ident.to_string();
        input.parse::<Token![:]>()?;
        let spec = input.parse::<FragSpec>()?;

        Ok(Fragment { ident, spec })
    }
}
impl Parse for FragSpec {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident = input.parse::<pm2::Ident>()?;
        if ident == "block" {
            Ok(FragSpec::Block)
        } else if ident == "expr" {
            Ok(FragSpec::Expr)
        } else if ident == "ident" {
            Ok(FragSpec::Ident)
        } else if ident == "item" {
            Ok(FragSpec::Item)
        } else if ident == "lifetime" {
            Ok(FragSpec::Lifetime)
        } else if ident == "literal" {
            Ok(FragSpec::Literal)
        } else if ident == "meta" {
            Ok(FragSpec::Meta)
        } else if ident == "pat" {
            Ok(FragSpec::Pattern)
        } else if ident == "path" {
            Ok(FragSpec::Path)
        } else if ident == "stmt" {
            Ok(FragSpec::Statement)
        } else if ident == "tt" {
            Ok(FragSpec::TokenTree)
        } else if ident == "ty" {
            Ok(FragSpec::Type)
        } else if ident == "vis" {
            Ok(FragSpec::Visibility)
        } else {
            Err(syn::Error::new(
                ident.span(),
                format!("unknown fragment specifier: {}", ident),
            ))
        }
    }
}

impl Parse for Transcribe {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(token::Dollar) && input.peek2(token::Paren) {
            Ok(Transcribe::Repetition(
                input.parse::<TranscribeRepetition>()?,
            ))
        } else if input.peek(token::Dollar) && input.peek2(syn::Ident) {
            input.parse::<token::Dollar>()?;
            Ok(Transcribe::Fragment(TranscribeFragment(
                input.parse::<pm2::Ident>()?.to_string(),
            )))
        } else {
            let tt = input.parse::<pm2::TokenTree>()?;
            match tt {
                pm2::TokenTree::Ident(ident) => Ok(Transcribe::Ident(ident)),
                pm2::TokenTree::Literal(literal) => Ok(Transcribe::Literal(literal)),
                pm2::TokenTree::Punct(punct) => Ok(Transcribe::Punct(punct)),
                pm2::TokenTree::Group(group) => Ok(Transcribe::Group(TranscribeGroup {
                    delimiter: group.delimiter(),
                    inner: syn::parse2::<TranscribeSeq>(group.stream())?,
                })),
            }
        }
    }
}

impl Parse for TranscribeRepetition {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<token::Dollar>()?; // $
        let inner;
        parenthesized!(inner in input);
        let inner = inner.parse::<TranscribeSeq>()?;
        let sep = input.parse::<Sep>()?;
        // absorb quantifier
        input.parse::<pm2::Punct>()?;
        Ok(TranscribeRepetition { inner, sep })
    }
}

impl Parse for TranscribeSeq {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut result = TranscribeSeq(vec![]);
        while !input.is_empty() {
            result.0.push(input.parse::<Transcribe>()?);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pm2::{Delimiter, Spacing, TokenTree};

    macro_rules! assert_match {
        ($(($input:expr) $binding:pat => $then:expr),+) => {{
            $(match &$input {
                $binding => $then,
                ref other => panic!("unexpected: {:?}", other),
            })+
        }};
    }

    #[test]
    fn frag() -> syn::Result<()> {
        spoor::init();

        let frag = syn::parse_str::<Fragment>("$elem:block")?;
        assert_eq!(frag.spec, FragSpec::Block);
        assert_eq!(frag.ident, "elem");
        Ok(())
    }

    #[test]
    fn frag_spec() -> syn::Result<()> {
        spoor::init();

        assert_eq!(syn::parse_str::<FragSpec>("block")?, FragSpec::Block);
        assert_eq!(syn::parse_str::<FragSpec>("expr")?, FragSpec::Expr);
        assert_eq!(syn::parse_str::<FragSpec>("ident")?, FragSpec::Ident);
        assert_eq!(syn::parse_str::<FragSpec>("item")?, FragSpec::Item);
        assert_eq!(syn::parse_str::<FragSpec>("lifetime")?, FragSpec::Lifetime);
        assert_eq!(syn::parse_str::<FragSpec>("literal")?, FragSpec::Literal);
        assert_eq!(syn::parse_str::<FragSpec>("meta")?, FragSpec::Meta);
        assert_eq!(syn::parse_str::<FragSpec>("pat")?, FragSpec::Pattern);
        assert_eq!(syn::parse_str::<FragSpec>("path")?, FragSpec::Path);
        assert_eq!(syn::parse_str::<FragSpec>("stmt")?, FragSpec::Statement);
        assert_eq!(syn::parse_str::<FragSpec>("tt")?, FragSpec::TokenTree);
        assert_eq!(syn::parse_str::<FragSpec>("ty")?, FragSpec::Type);
        assert_eq!(syn::parse_str::<FragSpec>("vis")?, FragSpec::Visibility);
        assert!(syn::parse_str::<FragSpec>("bees").is_err());
        Ok(())
    }

    #[test]
    fn matcher() -> syn::Result<()> {
        spoor::init();

        let seq = syn::parse_str::<MatcherSeq>(
            "ocelot + => $bees:ty { frog [] } $(tapir *)=>+ $(*)coati*",
        )?;

        assert_match! {
            (seq.0[0]) Matcher::Ident(ident) => assert_eq!(ident, "ocelot"),
            (seq.0[1]) Matcher::Punct(punct) => {
                assert_eq!(punct.as_char(), '+');
                assert_eq!(punct.spacing(), Spacing::Alone);
            },
            (seq.0[2]) Matcher::Punct(punct) => {
                assert_eq!(punct.as_char(), '=');
                assert_eq!(punct.spacing(), Spacing::Joint);
            },
            (seq.0[3]) Matcher::Punct(punct) => {
                assert_eq!(punct.as_char(), '>');
                assert_eq!(punct.spacing(), Spacing::Alone);
            },
            (seq.0[4]) Matcher::Fragment(frag) => {
                assert_eq!(frag.ident, "bees");
                assert_eq!(frag.spec, FragSpec::Type);
            },
            (seq.0[5]) Matcher::Group(group) => {
                assert_eq!(group.delimiter, Delimiter::Brace);
                assert_match!(
                    (group.inner.0[0]) Matcher::Ident(ident) => assert_eq!(ident, "frog"),
                    (group.inner.0[1]) Matcher::Group(group) => {
                        assert_eq!(group.delimiter, Delimiter::Bracket);
                        assert_eq!(group.inner.0.len(), 0);
                    }
                );
            },
            (seq.0[6]) Matcher::Repetition(rep) => assert_match! {
                (rep.inner.0[0]) Matcher::Ident(ident) => assert_eq!(ident, "tapir"),
                (rep.inner.0[1]) Matcher::Punct(punct) => assert_eq!(punct.as_char(), '*'),
                (rep.sep.0[0]) TokenTree::Punct(punct) => {
                    assert_eq!(punct.as_char(), '=');
                    assert_eq!(punct.spacing(), Spacing::Joint);
                },
                (rep.sep.0[1]) TokenTree::Punct(punct) => {
                    assert_eq!(punct.as_char(), '>');
                    assert_eq!(punct.spacing(), Spacing::Joint);
                }
            },
            (seq.0[7]) Matcher::Repetition(rep) => assert_match! {
                (rep.inner.0[0]) Matcher::Punct(punct) => assert_eq!(punct.as_char(), '*'),
                (rep.sep.0[0]) TokenTree::Ident(ident) => assert_eq!(ident, "coati")
            }
        }

        Ok(())
    }

    #[test]
    fn transcriber() -> syn::Result<()> {
        spoor::init();

        let seq = syn::parse_str::<TranscribeSeq>(
            "ocelot + => $bees { frog [] } $(tapir *)=>+ $(*)coati*",
        )?;

        assert_match! {
            (seq.0[0]) Transcribe::Ident(ident) => assert_eq!(ident, "ocelot"),
            (seq.0[1]) Transcribe::Punct(punct) => {
                assert_eq!(punct.as_char(), '+');
                assert_eq!(punct.spacing(), Spacing::Alone);
            },
            (seq.0[2]) Transcribe::Punct(punct) => {
                assert_eq!(punct.as_char(), '=');
                assert_eq!(punct.spacing(), Spacing::Joint);
            },
            (seq.0[3]) Transcribe::Punct(punct) => {
                assert_eq!(punct.as_char(), '>');
                assert_eq!(punct.spacing(), Spacing::Alone);
            },
            (seq.0[4]) Transcribe::Fragment(frag) => assert_eq!(frag.0, "bees"),
            (seq.0[5]) Transcribe::Group(group) => {
                assert_eq!(group.delimiter, Delimiter::Brace);
                assert_match!(
                    (group.inner.0[0]) Transcribe::Ident(ident) => assert_eq!(ident, "frog"),
                    (group.inner.0[1]) Transcribe::Group(group) => {
                        assert_eq!(group.delimiter, Delimiter::Bracket);
                        assert_eq!(group.inner.0.len(), 0);
                    }
                );
            },
            (seq.0[6]) Transcribe::Repetition(rep) => assert_match! {
                (rep.inner.0[0]) Transcribe::Ident(ident) => assert_eq!(ident, "tapir"),
                (rep.inner.0[1]) Transcribe::Punct(punct) => assert_eq!(punct.as_char(), '*'),
                (rep.sep.0[0]) TokenTree::Punct(punct) => {
                    assert_eq!(punct.as_char(), '=');
                    assert_eq!(punct.spacing(), Spacing::Joint);
                },
                (rep.sep.0[1]) TokenTree::Punct(punct) => {
                    assert_eq!(punct.as_char(), '>');
                    assert_eq!(punct.spacing(), Spacing::Joint);
                }
            },
            (seq.0[7]) Transcribe::Repetition(rep) => assert_match! {
                (rep.inner.0[0]) Transcribe::Punct(punct) => assert_eq!(punct.as_char(), '*'),
                (rep.sep.0[0]) TokenTree::Ident(ident) => assert_eq!(ident, "coati")
            }
        }

        Ok(())
    }

    #[test]
    fn full() -> syn::Result<()> {
        spoor::init();

        // let's get meta
        let mac = syn::parse_str::<MacroDef>(
            r#"
            macro_rules! assert_match {
                ($(($input:expr) $binding:pat => $then:expr),+) => {{
                    $(match &$input {
                        $binding => $then,
                        ref other => panic!("unexpected: {:?}", other),
                    })+
                }};
            }
        "#,
        );
        let mac = match mac {
            Err(e) => panic!("{}", e),
            Ok(mac) => mac,
        };
        assert_eq!(mac.ident, "assert_match");
        assert_eq!(mac.attrs.len(), 0);
        assert_eq!(mac.rules.len(), 1);
        assert_match!((mac.rules[0].matcher.0[0]) Matcher::Repetition(rep) => {
            assert_match!(
                (rep.sep.0[0]) TokenTree::Punct(punct) => {
                    assert_eq!(punct.as_char(), ',');
                },
                (rep.inner.0[0]) Matcher::Group(group) => {
                    assert_eq!(group.delimiter, Delimiter::Parenthesis);
                    assert_match!((group.inner.0[0]) Matcher::Fragment(frag) => {
                        assert_eq!(frag.ident, "input");
                        assert_eq!(frag.spec, FragSpec::Expr);
                    });
                }
            );
        });
        Ok(())
    }
}
