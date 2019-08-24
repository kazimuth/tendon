//! [Helpers for interfacing with external tools during the binding process.](https://www.youtube.com/watch?v=TjOb5uMJbIM)

use crate::{Map, Set};
use cargo_metadata::{Node, PackageId};
use std::collections::BinaryHeap;
use std::io;
use std::path::Path;
use std::process::Command;
use tracing::info;
use transgress_api::paths::AbsoluteCrate;

/// Run `cargo check` on target project to ensure well-formed input + dependencies.
pub fn check(path: &Path) -> io::Result<()> {
    info!("ensuring well-formed input");

    info!("$ cd {} && cargo check", path.display());
    let path_ = path.to_owned();
    let status = Command::new("cargo")
        .args(&["check"])
        .current_dir(path_)
        .status()?;

    if !status.success() {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "failed to run cargo check",
        ))
    } else {
        Ok(())
    }
}

/// Get an absolute crate identifier for a particular package.
pub fn get_crate(package: &cargo_metadata::Package) -> AbsoluteCrate {
    AbsoluteCrate {
        name: (&package.name[..]).into(),
        version: package.version.to_string().into(),
    }
}

/// Topologically sort a vec of cargo_metadata::Resolve Nodes using Kahn's algorithm, to create an
/// ordered vector of packageIDs such that traversing the list will never visit a node before its
/// dependencies.
pub fn topo_sort_nodes(nodes: &[Node]) -> Vec<PackageId> {
    let mut dependencies = nodes
        .iter()
        .map(|node| (&node.id, node.dependencies.iter().collect()))
        .collect::<Map<_, Set<_>>>();

    let mut result = vec![];

    // we only use a binary heap here for repeatability -- hashmap iteration will give slightly
    // different orderings every run otherwise, which is annoying
    let mut no_deps = nodes
        .iter()
        .filter(|node| node.dependencies.is_empty())
        .map(|node| &node.id)
        .collect::<BinaryHeap<_>>();

    let mut to_remove = vec![];

    while let Some(next) = no_deps.pop() {
        result.push(next.clone());

        for (node, deps) in &mut dependencies {
            if deps.remove(next) {
                if deps.is_empty() {
                    to_remove.push(node.clone());
                }
            }
        }

        for node in to_remove.drain(..) {
            dependencies.remove(node);
            no_deps.push(node);
        }
    }

    result
}
