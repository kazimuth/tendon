//! A syn parser for `macro_rules!`
//!
//!>    MacroRulesDefinition :
//!>       macro_rules ! IDENTIFIER MacroRulesDef
//!>    MacroRulesDef :
//!>          ( MacroRules ) ;
//!>       , [ MacroRules ] ;
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

use crate::{Error, Result};
use proc_macro2 as pm2;
use quote::{quote, ToTokens};
use syn::{
    self, parenthesized,
    parse::{self, Parse, ParseStream},
    token, Token,
};
#[derive(Debug)]
pub struct MacroDef {
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
pub struct TranscriberSeq(Vec<Transcriber>);

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
    pub sep: Vec<pm2::TokenTree>,
}
#[derive(Debug)]
pub struct Fragment {
    pub name: pm2::Ident,
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
    Pat,
    Path,
    Stmt,
    Tt,
    Ty,
    Vis,
}
#[derive(Debug)]
pub enum Transcriber {
    Tokens(pm2::TokenStream),
    Group {
        delimiter: pm2::Delimiter,
        inner: TranscriberSeq,
    },
    Fragment(pm2::Ident),
    Repeat {
        sep: Vec<pm2::TokenTree>,
        inner: TranscriberSeq,
    },
}

impl Parse for MacroDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let source = syn::ItemMacro::parse(input)?;
        // let mac = &source.mac;
        // let ident = source.ident.as_ref().ok_or(Error::NoMacroName)?.into();

        // Ok(MacroDef {
        //     ident,
        //     rules: vec![],
        // })
        unimplemented!()
    }
}
impl Parse for MacroRule {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        unimplemented!()
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
        let mut sep = vec![];

        // there's no easy way to parse "one token" (pm2 is too low-level)
        // so we just accept more than we should; rustc should already have weeded out incorrect things
        // for us
        while !input.peek(Token![*]) && !input.peek(Token![+]) && !input.peek(Token![?]) {
            let tt = input.parse::<pm2::TokenTree>()?;
            if let pm2::TokenTree::Group(ref group) = tt {
                Err(syn::Error::new(group.span(), "group in repetition sep???"))?;
            }
            sep.push(tt);
        }
        input.parse::<pm2::Punct>()?;
        Ok(Repetition { inner, sep })
    }
}
impl Parse for Fragment {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<token::Dollar>()?; // $
        let name = input.parse::<pm2::Ident>()?;
        input.parse::<Token![:]>()?;
        let spec = input.parse::<FragSpec>()?;

        Ok(Fragment { name, spec })
    }
}
impl Parse for FragSpec {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse::<pm2::Ident>()?;
        if name == "block" {
            Ok(FragSpec::Block)
        } else if name == "expr" {
            Ok(FragSpec::Expr)
        } else if name == "ident" {
            Ok(FragSpec::Ident)
        } else if name == "item" {
            Ok(FragSpec::Item)
        } else if name == "lifetime" {
            Ok(FragSpec::Lifetime)
        } else if name == "literal" {
            Ok(FragSpec::Literal)
        } else if name == "meta" {
            Ok(FragSpec::Meta)
        } else if name == "pat" {
            Ok(FragSpec::Pat)
        } else if name == "path" {
            Ok(FragSpec::Path)
        } else if name == "stmt" {
            Ok(FragSpec::Stmt)
        } else if name == "tt" {
            Ok(FragSpec::Tt)
        } else if name == "ty" {
            Ok(FragSpec::Ty)
        } else if name == "vis" {
            Ok(FragSpec::Vis)
        } else {
            Err(syn::Error::new(
                name.span(),
                format!("unknown fragment specifier: {}", name),
            ))
        }
    }
}

impl Parse for Transcriber {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        unimplemented!()
    }
}

impl Parse for TranscriberSeq {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pm2::{Delimiter, Ident, Punct, Spacing, TokenTree};

    macro_rules! assert_match {
        ($(($input:expr) $($type:ident::$variant:ident $(($binding:ident))?)? $($v:literal)? => $then:expr),+) => {{
            $(match $input {
                $($type::$variant$((ref $binding))?)? $($v)? => $then,
                ref other => panic!("unexpected: {:?}", other),
            })+
        }};
    }

    #[test]
    fn frag() -> syn::Result<()> {
        let frag = syn::parse_str::<Fragment>("$elem:block")?;
        assert_eq!(frag.spec, FragSpec::Block);
        assert_eq!(frag.name, "elem");
        Ok(())
    }
    #[test]
    fn frag_spec() -> syn::Result<()> {
        assert_eq!(syn::parse_str::<FragSpec>("block")?, FragSpec::Block);
        assert_eq!(syn::parse_str::<FragSpec>("expr")?, FragSpec::Expr);
        assert_eq!(syn::parse_str::<FragSpec>("ident")?, FragSpec::Ident);
        assert_eq!(syn::parse_str::<FragSpec>("item")?, FragSpec::Item);
        assert_eq!(syn::parse_str::<FragSpec>("lifetime")?, FragSpec::Lifetime);
        assert_eq!(syn::parse_str::<FragSpec>("literal")?, FragSpec::Literal);
        assert_eq!(syn::parse_str::<FragSpec>("meta")?, FragSpec::Meta);
        assert_eq!(syn::parse_str::<FragSpec>("pat")?, FragSpec::Pat);
        assert_eq!(syn::parse_str::<FragSpec>("path")?, FragSpec::Path);
        assert_eq!(syn::parse_str::<FragSpec>("stmt")?, FragSpec::Stmt);
        assert_eq!(syn::parse_str::<FragSpec>("tt")?, FragSpec::Tt);
        assert_eq!(syn::parse_str::<FragSpec>("ty")?, FragSpec::Ty);
        assert_eq!(syn::parse_str::<FragSpec>("vis")?, FragSpec::Vis);
        assert!(syn::parse_str::<FragSpec>("bees").is_err());
        Ok(())
    }
    #[test]
    fn matcher() -> syn::Result<()> {
        let seq =
            syn::parse_str::<MatcherSeq>("ocelot + => $bees:ty { frog } $(tapir *)=>+ $(*)coati*")?;

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
                assert_eq!(frag.name, "bees");
                assert_eq!(frag.spec, FragSpec::Ty);
            },
            (seq.0[5]) Matcher::Group(group) => {
                assert_eq!(group.delimiter, Delimiter::Brace);
                assert_match!((group.inner.0[0]) Matcher::Ident(ident) => {
                    assert_eq!(ident, "frog");
                });
            },
            (seq.0[6]) Matcher::Repetition(rep) => assert_match! {
                (rep.inner.0[0]) Matcher::Ident(ident) => assert_eq!(ident, "tapir"),
                (rep.inner.0[1]) Matcher::Punct(punct) => assert_eq!(punct.as_char(), '*'),
                (rep.sep[0]) TokenTree::Punct(punct) => {
                    assert_eq!(punct.as_char(), '=');
                    assert_eq!(punct.spacing(), Spacing::Joint);
                },
                (rep.sep[1]) TokenTree::Punct(punct) => {
                    assert_eq!(punct.as_char(), '>');
                    assert_eq!(punct.spacing(), Spacing::Joint);
                }
            },
            (seq.0[7]) Matcher::Repetition(rep) => assert_match! {
                (rep.inner.0[0]) Matcher::Punct(punct) => assert_eq!(punct.as_char(), '*'),
                (rep.sep[0]) TokenTree::Ident(ident) => assert_eq!(ident, "coati")
            }
        }

        Ok(())
    }
}
