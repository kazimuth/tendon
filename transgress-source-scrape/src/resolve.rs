//! Name resolution and macro expansion.
// https://rust-lang.github.io/rustc-guide/name-resolution.html

// TODO purge:

// TODO limit memory usage

// TODO macro resolution order / scope?

// TODO "macro" kw, whenever that exists

// TODO handle no_link,no_std,no_prelude

use crate::{Error, Result};
use cargo_metadata::{CargoOpt, Metadata, MetadataCommand, Node, Package, PackageId};
use quote::ToTokens;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use syn;
use tracing::{info, info_span};
use transgress_api::Ident;

pub mod item_expand;

pub struct Resolver {
    /// The root of the project we're scraping.
    pub project_root: PathBuf,
    /// The workspace root of the project; same as project_root unless we're in a workspace.
    pub workspace_root: PathBuf,
    /// The list of all dependencies of this project.
    /// Note that each dependency may be instantiated multiple times with different feature sets; see nodes
    /// for the actual dependency graph.
    pub packages: HashMap<PackageId, Package>,
    /// The dependency graph, tracking package instantiations
    pub resolve: HashMap<PackageId, Node>,
    /// The root project we're examining
    pub root: PackageId,
}

impl Resolver {
    pub fn new(project_root: PathBuf) -> Result<Resolver> {
        let project = project_root.clone();
        info!("Collecting cargo metadata");
        let metadata = MetadataCommand::new()
            .current_dir(&project)
            .manifest_path(&project.join("Cargo.toml"))
            .features(CargoOpt::AllFeatures)
            .exec()?;

        let Metadata {
            mut packages,
            resolve,
            workspace_root,
            ..
        } = metadata;
        let packages: HashMap<_, _> = packages
            .drain(..)
            .map(|package| (package.id.clone(), package))
            .collect();
        let mut resolve = resolve.ok_or(Error::ResolveFailed)?;
        let root = resolve.root.ok_or(Error::ResolveFailed)?;
        let resolve = resolve
            .nodes
            .drain(..)
            .map(|node| (node.id.clone(), node))
            .collect();

        info!("root package {:?}", root.repr);

        Ok(Resolver {
            packages,
            root,
            resolve,
            project_root,
            workspace_root,
        })
    }

    pub fn parse_crate(&self, id: PackageId) -> Result<()> {
        // look up in resolve?
        let span = info_span!("parse_crate", crate_ = &id.repr.split(" ").next().unwrap());
        let _entered = span.enter();

        info!("parsing crate {}", id.repr);
        let package = &self.packages[&id];
        let lib_target = &package
            .targets
            .iter()
            .find(|t| t.kind.iter().find(|k| *k == "lib").is_some())
            .ok_or(Error::Other {
                cause: "no lib target in crate",
            })?;
        let root = lib_target.src_path.clone();
        self.parse_module(root, ResolvedPath::new(&id, ""))
    }

    pub fn parse_module(&self, root: PathBuf, path: ResolvedPath) -> Result<()> {
        info!("parsing {} (`{}`)", pretty(&path), root.display());

        let mut file = File::open(root)?;
        let mut source = String::new();
        file.read_to_string(&mut source)?;

        let source = syn::parse_str::<syn::File>(&source)?;

        for item in &source.items {
            match item {
                syn::Item::Use(use_) => {
                    info!("use {}", use_.tree.clone().into_token_stream());
                }
                syn::Item::ExternCrate(crate_) => self.skip("crate", path.join(&crate_.ident)),
                syn::Item::Static(static_) => self.skip("static", path.join(&static_.ident)),
                syn::Item::Const(const_) => self.skip("const", path.join(&const_.ident)),
                syn::Item::Fn(fn_) => self.skip("fn", path.join(&fn_.ident)),
                syn::Item::Mod(mod_) => self.skip("mod", path.join(&mod_.ident)),
                syn::Item::Type(type_) => self.skip("type", path.join(&type_.ident)),
                syn::Item::Existential(existential_) => {
                    self.skip("existential", path.join(&existential_.ident))
                }
                syn::Item::Struct(struct_) => self.skip("struct", path.join(&struct_.ident)),
                syn::Item::Enum(enum_) => self.skip("enum", path.join(&enum_.ident)),
                syn::Item::Union(union_) => self.skip("union", path.join(&union_.ident)),
                syn::Item::Trait(trait_) => self.skip("trait", path.join(&trait_.ident)),
                syn::Item::TraitAlias(alias_) => self.skip("alias", path.join(&alias_.ident)),
                syn::Item::Impl(impl_) => {
                    info!("impl: {}", impl_.into_token_stream());
                }
                syn::Item::Macro(macro_rules_) => {
                    if let Some(ident) = &macro_rules_.ident {
                        self.skip("macro_rules", path.join(ident))
                    }
                }
                syn::Item::Macro2(macro2_) => self.skip("macro2_", path.join(&macro2_.ident)),
                syn::Item::ForeignMod(_foreign_mod_) => self.skip("foreign_mod", path.clone()),
                syn::Item::Verbatim(_verbatim_) => self.skip("verbatim", path.clone()),
            }
        }

        // scope to target crate?

        // https://doc.rust-lang.org/stable/reference/items/modules.html
        // mod q; -> q/mod.rs; q.rs
        //
        // #[path="z.rs"] mod q -> z.rs
        // #[path="bees"] mod wasps { mod queen; } -> bees/queen.rs, bees/queen/mod.rs

        // cfg-attrs

        // prelude

        // check edition
        Ok(())
    }

    pub fn skip(&self, kind: &str, path: ResolvedPath) {
        info!("skipping {} {}", kind, pretty(&path));
    }
}

fn pretty(path: &ResolvedPath) -> String {
    format!(
        "{}::{}",
        path.id.repr.split(" ").next().unwrap(),
        path.path.join("::")
    )
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct ResolvedPath {
    /// The crate instantiation this path comes from.
    pub id: PackageId,
    /// The path of the item, rooted within that package.
    pub path: Vec<Ident>,
}
impl ResolvedPath {
    fn new(id: &PackageId, path: impl AsRef<str>) -> Self {
        ResolvedPath {
            id: id.clone(),
            path: path
                .as_ref()
                .split("::")
                .filter_map(|s| if s.len() > 0 { Some(s.into()) } else { None })
                .collect(),
        }
    }
    fn join(&self, elem: impl Into<Ident>) -> Self {
        let elem = elem.into();
        assert!(!elem.contains("::"));

        let id = self.id.clone();
        let mut path = self.path.clone();
        path.push(elem.into());

        ResolvedPath { id, path }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_path() {
        let empty: ResolvedPath = ResolvedPath::new(
            &PackageId {
                repr: "test_package".into(),
            },
            "",
        );
        assert_eq!(empty.path, Vec::<Ident>::new());
        let empty: ResolvedPath = ResolvedPath::new(
            &PackageId {
                repr: "test_package".into(),
            },
            "::",
        );
        assert_eq!(empty.path, Vec::<Ident>::new());

        let empty: ResolvedPath = ResolvedPath::new(
            &PackageId {
                repr: "test_package".into(),
            },
            "a::b",
        );
        assert_eq!(empty.path, vec!["a".into(), "b".into()]);
    }
}
