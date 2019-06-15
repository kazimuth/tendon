//! Name resolution and macro expansion.
//! Works asynchronously and memoizes as it goes in order to achieve MAXIMUM HARDWARE EXPLOITATION.

// TODO purge:
#![allow(unused)]

use crate::{Error, Result};
use cargo_metadata::{CargoOpt, Metadata, MetadataCommand, Node, Package, PackageId};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio_trace::info;
use unthwart::unthwarted;

pub mod item_expand;
pub mod registry;

// TODO limit memory usage

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
    nodes: HashMap<PackageId, Node>,
    /// The root project we're examining
    root: PackageId,
    /// Files loaded from the filesystem.
    files: RwLock<HashMap<PathBuf, syn::File>>,
    /// Where ResolvedPaths can be found in the filesystem.
    modules: RwLock<HashMap<ResolvedPath, PathBuf>>,
}

impl Resolver {
    pub async fn new(project_root: PathBuf) -> Result<Resolver> {
        let project = project_root.clone();
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
        let packages = packages
            .drain(..)
            .map(|package| (package.id.clone(), package))
            .collect();
        let mut resolve = resolve.ok_or(Error::ResolveFailed)?;
        let root = resolve.root.ok_or(Error::ResolveFailed)?;
        let nodes = resolve
            .nodes
            .drain(..)
            .map(|node| (node.id.clone(), node))
            .collect();

        let files = RwLock::new(HashMap::new());
        let modules = RwLock::new(HashMap::new());

        info!("root package {:?}", root.repr);

        Ok(Resolver {
            packages,
            root,
            nodes,
            project_root,
            workspace_root,
            files,
            modules,
        })
    }

    async fn resolve<'a>(&'a self, path: &'a syn::Path) -> Result<ResolvedPath> {
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

/// Get the source for some module.

/// A scraped fn.
struct Fn {}

/// A scraped type.
struct Type {}

/// A scraped trait.
struct Trait {}

/// A scraped const.
struct Const {}

/// A scraped static.
struct Static {}
