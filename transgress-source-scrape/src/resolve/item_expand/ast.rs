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
#[derive(Debug)]
pub struct MacroDef {
    pub attrs: Vec<syn::Attribute>,
    pub ident: pm2::Ident,
    pub rules: Vec<MacroRule>,
}
#[derive(Debug)]
pub struct MacroRule {
    pub matcher: MatcherSeq,
    pub transcriber: TranscriberSeq,
}

#[derive(Debug)]
pub struct MatcherSeq(pub Vec<Matcher>);
#[derive(Debug)]
pub struct TranscriberSeq(pub Vec<Transcriber>);

#[derive(Debug)]
pub enum Matcher {
    Repetition(Repetition),
    Fragment(Fragment),
    Group(Group),
    Ident(pm2::Ident),
    Literal(pm2::Literal),
    Punct(pm2::Punct),
}
#[derive(Debug)]
pub struct Repetition {
    pub inner: MatcherSeq,
    pub sep: Sep,
}
#[derive(Debug)]
pub struct Sep(pub Vec<pm2::TokenTree>);

#[derive(Debug)]
pub struct Fragment {
    pub ident: pm2::Ident,
    pub spec: FragSpec,
}
#[derive(Debug)]
pub struct Group {
    pub delimiter: pm2::Delimiter,
    pub inner: MatcherSeq,
}
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
#[derive(Debug)]
pub enum Transcriber {
    // TODO: can be a false match?
    Fragment(pm2::Ident),
    Repetition(TranscribeRepetition),
    Group(TranscribeGroup),
    Ident(pm2::Ident),
    Literal(pm2::Literal),
    Punct(pm2::Punct),
}
#[derive(Debug)]
pub struct TranscribeRepetition {
    sep: Sep,
    inner: TranscriberSeq,
}

#[derive(Debug)]
pub struct TranscribeGroup {
    delimiter: pm2::Delimiter,
    inner: TranscriberSeq,
}

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

        let rules = syn::parse2::<MacroRules>(mac.tts)?.0;

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
        let transcriber = syn::parse2::<TranscriberSeq>(transcriber.stream())?;

        input.parse::<Token![;]>()?;

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
        // absorb quantifier
        // TODO keep? can it affect parsing?
        input.parse::<pm2::Punct>()?;

        Ok(Repetition { inner, sep })
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

impl Parse for Transcriber {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(token::Dollar) && input.peek2(token::Paren) {
            Ok(Transcriber::Repetition(
                input.parse::<TranscribeRepetition>()?,
            ))
        } else if input.peek(token::Dollar) && input.peek2(syn::Ident) {
            input.parse::<token::Dollar>()?;
            Ok(Transcriber::Fragment(input.parse::<pm2::Ident>()?))
        } else {
            let tt = input.parse::<pm2::TokenTree>()?;
            match tt {
                pm2::TokenTree::Ident(ident) => Ok(Transcriber::Ident(ident)),
                pm2::TokenTree::Literal(literal) => Ok(Transcriber::Literal(literal)),
                pm2::TokenTree::Punct(punct) => Ok(Transcriber::Punct(punct)),
                pm2::TokenTree::Group(group) => Ok(Transcriber::Group(TranscribeGroup {
                    delimiter: group.delimiter(),
                    inner: syn::parse2::<TranscriberSeq>(group.stream())?,
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
        let inner = inner.parse::<TranscriberSeq>()?;
        let sep = input.parse::<Sep>()?;
        // absorb quantifier
        input.parse::<pm2::Punct>()?;
        Ok(TranscribeRepetition { inner, sep })
    }
}

impl Parse for TranscriberSeq {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut result = TranscriberSeq(vec![]);
        while !input.is_empty() {
            result.0.push(input.parse::<Transcriber>()?);
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

        let seq = syn::parse_str::<TranscriberSeq>(
            "ocelot + => $bees { frog [] } $(tapir *)=>+ $(*)coati*",
        )?;

        assert_match! {
            (seq.0[0]) Transcriber::Ident(ident) => assert_eq!(ident, "ocelot"),
            (seq.0[1]) Transcriber::Punct(punct) => {
                assert_eq!(punct.as_char(), '+');
                assert_eq!(punct.spacing(), Spacing::Alone);
            },
            (seq.0[2]) Transcriber::Punct(punct) => {
                assert_eq!(punct.as_char(), '=');
                assert_eq!(punct.spacing(), Spacing::Joint);
            },
            (seq.0[3]) Transcriber::Punct(punct) => {
                assert_eq!(punct.as_char(), '>');
                assert_eq!(punct.spacing(), Spacing::Alone);
            },
            (seq.0[4]) Transcriber::Fragment(frag) => assert_eq!(frag, "bees"),
            (seq.0[5]) Transcriber::Group(group) => {
                assert_eq!(group.delimiter, Delimiter::Brace);
                assert_match!(
                    (group.inner.0[0]) Transcriber::Ident(ident) => assert_eq!(ident, "frog"),
                    (group.inner.0[1]) Transcriber::Group(group) => {
                        assert_eq!(group.delimiter, Delimiter::Bracket);
                        assert_eq!(group.inner.0.len(), 0);
                    }
                );
            },
            (seq.0[6]) Transcriber::Repetition(rep) => assert_match! {
                (rep.inner.0[0]) Transcriber::Ident(ident) => assert_eq!(ident, "tapir"),
                (rep.inner.0[1]) Transcriber::Punct(punct) => assert_eq!(punct.as_char(), '*'),
                (rep.sep.0[0]) TokenTree::Punct(punct) => {
                    assert_eq!(punct.as_char(), '=');
                    assert_eq!(punct.spacing(), Spacing::Joint);
                },
                (rep.sep.0[1]) TokenTree::Punct(punct) => {
                    assert_eq!(punct.as_char(), '>');
                    assert_eq!(punct.spacing(), Spacing::Joint);
                }
            },
            (seq.0[7]) Transcriber::Repetition(rep) => assert_match! {
                (rep.inner.0[0]) Transcriber::Punct(punct) => assert_eq!(punct.as_char(), '*'),
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
