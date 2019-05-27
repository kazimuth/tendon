//! A library for converting compiled rustdoc output into a
//! programmatic description of a module's API.
//! Believe it or not, this is actually the best way to do this right now.
//! We use a variety of different parsers and scrapers to get the job done,
//! including "soup" for html and "syn" for embedded rust code.
//! I won't say "abandon all hope ye who enter here" because we try to keep this
//! reasonably sane, but it's still a scraper, so you should abandon at least
//! a little bit of hope.

#![allow(dead_code)]

use cargo_metadata::MetadataCommand;
use lazy_static::lazy_static;
use quote::quote;
use regex::Regex;
use soup::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Command;

use log::{info, warn};

#[derive(Debug)]
pub struct Api {
    pub items: Vec<Item>,
}

/// Parse the API of a crate.
pub fn extract_for_crate<P: AsRef<Path>>(dir: P) -> Result<Api, Error> {
    let dir = dir.as_ref();

    let _metadata = MetadataCommand::new().current_dir(dir).exec().unwrap();

    // TODO: handle rustc / rustdoc versions

    info!("building cargo docs to scrape...");

    // run rustdoc
    // TODO: feature handling
    let status = Command::new("cargo")
        .args(&["doc"])
        .current_dir(dir)
        .status()?;
    assert!(status.success());

    let package = dir.file_name().unwrap().to_string_lossy();
    info!("package: {:?}", package);

    // TODO: cargo env var handling, weirder setups
    let docdir = dir.join("target").join("doc");
    let workspace_docdir = dir
        .parent()
        .expect("stop doing things at /")
        .join("target")
        .join("doc");

    let docdir = if docdir.is_dir() {
        docdir
    } else if workspace_docdir.is_dir() {
        workspace_docdir
    } else {
        panic!("can't find target")
    };

    let mut queue = VecDeque::new();
    let mut results = vec![];
    let mut visited = HashSet::new();

    let first = docdir.join(&fix_module_name(&package)).join("index.html");

    queue.push_back(WorkItem::Module(first));

    while let Some(next) = queue.pop_front() {
        if visited.contains(&next) {
            continue;
        }
        visited.insert(next.clone());

        let (result, path) = match next {
            WorkItem::Module(p) => (parse_module(&p, &mut queue, &mut results), p),
            WorkItem::Struct(p) => (parse_struct(&p, &mut queue, &mut results), p),
            e => {
                warn!("unknown workitem: {:?}", e);
                continue;
            }
        };
        if let Err(e) = result {
            warn!("while scraping `{}`: {}", path.display(), e);
        }
    }

    let result = Api { items: results };

    Ok(result)
}

/// Parse a module .html file.
fn parse_module(
    path: &Path,
    queue: &mut VecDeque<WorkItem>,
    results: &mut Vec<Item>,
) -> Result<(), Error> {
    info!("module: {:?}", path);

    let parent = path.parent().ok_or(Error::WeirdFs)?;

    let parser = soup_(path)?;

    for node in parser.tag("a").class("struct").find_all() {
        let link = node.get("href").ok_or(Error::MissingAttribute {
            attr: "href on `a.struct`",
        })?;
        if link.len() > 0 {
            queue.push_back(WorkItem::Struct(parent.join(link)));
        }
    }

    for node in parser.tag("a").class("mod").find_all() {
        let link = node.get("href").ok_or(Error::MissingAttribute {
            attr: "href on `a.mod`",
        })?;
        if link.len() > 0 {
            queue.push_back(WorkItem::Module(parent.join(link)));
        }
    }
    for node in parser.tag("a").class("fn").find_all() {
        let link = node.get("href").ok_or(Error::MissingAttribute {
            attr: "href on `a.fn`",
        })?;
        if link.len() > 0 {
            queue.push_back(WorkItem::Fn_(parent.join(link)));
        }
    }

    let name = parser
        .tag("h1")
        .class("fqn")
        .find()
        .ok_or(Error::MissingElement { elt: "`h1.fqn`" })?
        .tag("span")
        .class("in-band")
        .find()
        .ok_or(Error::MissingElement {
            elt: "`h1.fqn > span.in-band`",
        })?
        .tag("span")
        .class("in-band")
        .find()
        .ok_or(Error::MissingElement {
            elt: "`h1.fqn > span.in-band > a.mod`",
        })?
        .text();

    let is_crate = name.starts_with("Crate"); // otherwise, module
    let name = remove_upto_space(&name);

    let name = syn::parse_str(&name)?;

    results.push(Item::Module(Module { is_crate, name }));

    Ok(())
}

/// Parse a struct .html file.
fn parse_struct(
    path: &Path,
    _: &mut VecDeque<WorkItem>,
    results: &mut Vec<Item>,
) -> Result<(), Error> {
    info!("struct: {:?}", path);

    let parser = soup_(path)?;

    let path = get_path(&parser)?;

    let (src, resolved) = get_source(&parser)?;
    let code: syn::DeriveInput = syn::parse_str(&src)?;
    // TODO: modify code to use fully-expanded paths, or UNKNOWN for non-known paths

    info!(
        "  `{}`: `{}`",
        quote!(#path).to_string(),
        quote!(#code).to_string()
    );

    results.push(Item::Struct(Struct {
        path: path.clone(),
        ident: path
            .segments
            .last()
            .ok_or(Error::MalformedRustdoc)?
            .value()
            .ident
            .clone(),
    }));

    Ok(())
}

/// Lookup the path from a title in a rustdoc document.
/// Rustdoc formats the title in the form:
/// <h1 class="fqn"><span class="in-band">Struct <a href="...">module</a>::<a href="..."Item"></span>...</h1>
fn get_path(parser: &Soup) -> Result<syn::Path, Error> {
    // of the form: "Struct module::mod::Thing" or "Enum module::mod::Thing"
    let path = parser
        .tag("h1")
        .class("fqn")
        .find()
        .ok_or(Error::MissingElement { elt: "`h1.fqn`" })?
        .tag("span")
        .class("in-band")
        .find()
        .ok_or(Error::MissingElement {
            elt: "`h1.fqn > span.in-band`",
        })?
        .text();
    let path = remove_upto_space(&path);

    // parse w/ syn
    Ok(syn::parse_str(&path)?)
}

const NIGHTLY_PREFIX: &'static str = "https://doc.rust-lang.org/nightly/";

/// Parse the embedded source for an item, using rustdoc's links to resolve names.
/// TODO: switch to harvesting from .rs.html files?
///       ...when feasible, items from macros won't work that way...
fn get_source(parser: &Soup) -> Result<(String, HashMap<syn::Path, syn::Path>), Error> {
    let node = parser
        .tag("div")
        .class("type-decl")
        .find()
        .ok_or(Error::MissingElement {
            elt: "`div.type-decl`",
        })?
        .tag("pre")
        .find()
        .ok_or(Error::MissingElement {
            elt: "`div.type-decl > pre`",
        })?;

    let mut result = HashMap::new();
    for child in node.tag("a").find_all() {
        let text = syn::parse_str(&child.text())?;
        let href = child.get("href").ok_or(Error::MissingAttribute {
            attr: "href on `div.type-decl > pre > a`".into(),
        })?;

        result.insert(text, url_to_rust_path(&href)?);
    }

    Ok((node.text(), result))
}

fn url_to_rust_path(path: &str) -> Result<syn::Path, Error> {
    const NIGHTLY: &'static str = "https://doc.rust-lang.org/nightly/";
    let path = if path.starts_with("../") {
        &path[3..]
    } else if path.starts_with(NIGHTLY) {
        &path[NIGHTLY.len()..]
    } else {
        return Err(Error::UnknownPathForm { path: path.into() });
    };

    // using regex to parse URLs is strictly wrong,
    // but rustdoc uses a subset of URLs so we should be okay

    lazy_static! {
        // parse things of the form `test_crate/z/struct.InMod.html`
        static ref RE: Regex = Regex::new(
            r"(?x)
            ^(.*) /
            (primitive|struct|trait|type|enum) # file type 
            \.
            ([a-zA-Z0-9_]*)
            \.
            html
            $" 
        )
        .unwrap();
    };
    let caps = RE
        .captures(path)
        .ok_or(Error::UnknownPathForm { path: path.into() })?;

    let prefix = caps[1].replace("/", "::");
    let _kind = &caps[2];
    let name = &caps[3];

    let path = format!("{}::{}", prefix, name);
    let path = syn::parse_str(&path)?;
    Ok(path)
}

// TODO: switch to inferring kind from URL?
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
enum WorkItem {
    Module(PathBuf),
    Struct(PathBuf),
    Fn_(PathBuf),
    Trait(PathBuf),
}

#[derive(Debug)]
pub enum Item {
    Module(Module),
    Struct(Struct),
}

#[derive(Debug)]
pub struct Module {
    is_crate: bool,
    name: syn::Path,
}

#[derive(Debug)]
pub struct Struct {
    path: syn::Path,
    ident: syn::Ident,
}

custom_error::custom_error! { pub Error
    Io { source: std::io::Error }           = "io error: {source}",
    Parse { source: syn::Error }            = "rust parsing error: {source}",
    MalformedRustdoc                        = "rustdoc formatting does not match expectations",
    MissingAttribute { attr: StaticStr }    = "missing attribute in rustdoc: {attr}",
    MissingElement { elt: StaticStr }       = "missing element in rustdoc: {elt}",
    WeirdFs                                 = "weird fs layout",
    UnknownPathForm { path: String }        = "unknown path form {path}",
}

type StaticStr = &'static str;

fn remove_upto_space(s: &str) -> String {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"[^ ]* ").unwrap();
    }
    RE.replace_all(s, "").into()
}

fn soup_(path: &Path) -> Result<Soup, Error> {
    Ok(Soup::from_reader(std::io::BufReader::new(
        std::fs::File::open(path)?,
    ))?)
}

fn fix_module_name(name: &str) -> String {
    name.replace("-", "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_conversions() -> Result<(), Error> {
        assert_eq!(
            url_to_rust_path("https://doc.rust-lang.org/nightly/std/convert/trait.TryFrom.html")?,
            syn::parse_str("std::convert::TryFrom")?
        );
        assert_eq!(
            url_to_rust_path("https://doc.rust-lang.org/nightly/std/convert/trait.TryFrom.html")?,
            syn::parse_str("std::convert::TryFrom")?
        );
        Ok(())
    }
}
