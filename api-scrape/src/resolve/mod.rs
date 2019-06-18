//! Name resolution and macro expansion.
//! Works asynchronously and memoizes as it goes in order to achieve MAXIMUM HARDWARE EXPLOITATION.

// TODO purge:
#![allow(unused)]

// TODO limit memory usage

// TODO macro resolution order / scope?

// TODO "macro" kw, whenever that exists

use crate::{Error, Result};
use cargo_metadata::{CargoOpt, Metadata, MetadataCommand, Node, Package, PackageId};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio_trace::info;
use unthwart::unthwarted;

pub mod item_expand;

pub struct Resolver {
    /// The root of the project we're scraping.
    project_root: PathBuf,
    /// The workspace root of the project; same as project_root unless we're in a workspace.
    workspace_root: PathBuf,
    /// The list of all dependencies of this project.
    /// Note that each dependency may be instantiated multiple times with different feature sets; see nodes
    /// for the actual dependency graph.
    packages: HashMap<PackageId, Package>,
    /// The dependency graph, tracking package instantiations
    resolve: HashMap<PackageId, Node>,
    /// The root project we're examining
    root: PackageId,
    /// Where ResolvedPaths can be found in the filesystem.
    modules: RwLock<HashMap<ResolvedPath, PathBuf>>,
}

impl Resolver {
    pub async fn new(project_root: PathBuf) -> Result<Resolver> {
        let project = project_root.clone();
        info!("Collecting cargo metadata");
        let mut metadata = unthwarted! {
            MetadataCommand::new()
                .current_dir(&project)
                .manifest_path(&project.join("Cargo.toml"))
                .features(CargoOpt::AllFeatures)
                .exec()?
        };
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

        let modules = RwLock::new(HashMap::new());

        for package in packages.values() {
            info!("{}", package.name);
        }

        info!("root package {:?}", root.repr);

        Ok(Resolver {
            packages,
            root,
            resolve,
            project_root,
            workspace_root,
            modules,
        })
    }

    fn resolve<'a>(&'a self, id: &PackageId, path: &'a syn::Path) -> Result<ResolvedPath> {
        // scope to target crate?

        // look up in resolve

        // https://doc.rust-lang.org/stable/reference/items/modules.html
        // mod q; -> q/mod.rs; q.rs
        //
        // #[path="z.rs"] mod q -> z.rs
        // #[path="bees"] mod wasps { mod queen; } -> bees/queen.rs, bees/queen/mod.rs

        // cfg-attrs

        // prelude

        // check edition
        // let count = &package.targets.iter().filter(|t| t.kind.iter().find(|k| *k == "lib").is_some()).count();
        unimplemented!();
    }

    async fn get_module(path: &ResolvedPath) -> Result<(Vec<syn::Attribute>, Vec<syn::Item>)> {
        unimplemented!()
    }
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct ResolvedPath {
    /// The crate instantiation this path comes from.
    pub package: PackageId,
    /// The path of the item, rooted within that package.
    pub path: Vec<String>,
}

///
pub struct BarePath(Box<str>);
