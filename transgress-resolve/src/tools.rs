//! [Helpers for interfacing with external tools during the binding process.](https://www.youtube.com/watch?v=TjOb5uMJbIM)

use crate::resolver::CrateData;
use crate::{Map, Set};
use cargo_metadata::{Metadata, Node, Package, PackageId};
use std::collections::{BinaryHeap, HashMap};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path as FsPath, PathBuf};
use std::process::Command;
use tracing::{info, warn};
use transgress_api::idents::Ident;
use transgress_api::paths::AbsoluteCrate;

/// Run `cargo check` on target project to ensure well-formed input + dependencies.
pub fn check(path: &FsPath) -> io::Result<()> {
    info!("ensuring well-formed input");

    // TODO different `cargo` invocations?

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
/// We strip out all "-"s here.
pub fn lower_absolute_crate(package: &cargo_metadata::Package) -> AbsoluteCrate {
    AbsoluteCrate {
        name: package.name[..].replace("-", "_").into(),
        version: package.version.to_string().into(),
    }
}

/// Get the sources dir from the sysroot active for some path.
pub fn sources_dir(target_dir: &FsPath) -> io::Result<PathBuf> {
    info!(
        "$ cd {} && cargo rustc -- --print-sysroot",
        target_dir.display()
    );

    // TODO different `cargo` invocations?

    let target_dir = target_dir.to_owned();
    let sysroot = Command::new("cargo")
        .args(&["rustc", "--", "--print", "sysroot"])
        .current_dir(target_dir)
        .output()?
        .stdout;

    // TODO does this work on windows?
    // TODO non-UTF8 paths

    let sysroot = PathBuf::from(String::from_utf8_lossy(&sysroot).trim());
    let sources = sysroot
        .join("lib")
        .join("rustlib")
        .join("src")
        .join("rust")
        .join("src");

    info!("sysroot: {}", sysroot.display());
    info!("sources: {}", sources.display());

    if !fs::metadata(&sources)?.is_dir() {
        // TODO run this automatically?
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "can't find rust sources, did you run rustup component add rust-src?",
        ));
    }

    Ok(sources)
}

/// Get the sysroot active for some crate.
/// This is necessary to harvest data from std / core.
/// TODO: no_std, pre-distribute metadata for these crates?
pub fn add_rust_sources(
    crates: &mut Map<AbsoluteCrate, CrateData>,
    target_dir: &FsPath,
) -> io::Result<()> {
    info!("finding libstd + libcore + liballoc");

    let sources = sources_dir(target_dir)?;

    let libcore = AbsoluteCrate {
        name: "core".into(),
        version: "0.0.0".into(),
    };
    let libstd = AbsoluteCrate {
        name: "std".into(),
        version: "0.0.0".into(),
    };
    let liballoc = AbsoluteCrate {
        name: "alloc".into(),
        version: "0.0.0".into(),
    };

    for crate_ in crates.values_mut() {
        crate_.deps.insert("core".into(), libcore.clone());
        crate_.deps.insert("alloc".into(), liballoc.clone());
        crate_.deps.insert("std".into(), libstd.clone());
    }

    crates.insert(
        libcore.clone(),
        CrateData {
            deps: Map::default(),
            is_proc_macro: false,
            cargo_source: None,
            src_root: sources.join("libcore"),
            manifest_path: sources.join("libcore").join("Cargo.toml"),
            features: vec![],
        },
    );

    let mut deps = Map::default();
    deps.insert("core".into(), libcore.clone());
    crates.insert(
        liballoc.clone(),
        CrateData {
            deps: deps.clone(),
            is_proc_macro: false,
            cargo_source: None,
            src_root: sources.join("liballoc"),
            manifest_path: sources.join("liballoc").join("Cargo.toml"),
            features: vec![],
        },
    );

    deps.insert("alloc".into(), liballoc.clone());
    crates.insert(
        libstd.clone(),
        CrateData {
            deps,
            is_proc_macro: false,
            cargo_source: None,
            src_root: sources.join("libstd"),
            manifest_path: sources.join("libstd").join("Cargo.toml"),
            features: vec![],
        },
    );

    Ok(())
}

/// Compute the transitive dependencies of the target crate (to avoid extra work in workspaces).
pub fn transitive_dependencies(
    target_crate: &AbsoluteCrate,
    crates: &Map<AbsoluteCrate, CrateData>,
) -> Set<AbsoluteCrate> {
    let mut dependencies: Map<AbsoluteCrate, Set<&AbsoluteCrate>> = crates
        .keys()
        .map(|crate_| (crate_.clone(), crates[crate_].deps.values().collect()))
        .collect();

    // find transitive dependencies of target crate (to avoid extra work in workspaces)
    let mut transitive_deps = Set::default();
    transitive_deps.insert(target_crate.clone());
    let mut to_walk = vec![target_crate];
    while let Some(next) = to_walk.pop() {
        for dep in &dependencies[next] {
            if transitive_deps.insert((*dep).clone()) {
                to_walk.push(*dep);
            }
        }
    }

    transitive_deps
}

/// Lower information from `cargo_metadata` to an intelligible form.
/// Note that `cargo_metadata` stores data in two places, as `Package`s and as `Node`s.
/// A `Package` is all of the metadata for a crate, as pulled from a cargo.toml, including all
/// possible features and dependencies; a `Node` is a specific instantiation of a package with some
/// set of features. Every `Package` can only have one corresponding `Node`.
pub fn lower_crates(metadata: &Metadata) -> Map<AbsoluteCrate, CrateData> {
    let mut result = Map::default();
    let packages = metadata
        .packages
        .iter()
        .map(|package| (&package.id, package))
        .collect::<Map<_, _>>();
    let abs_crates = metadata
        .packages
        .iter()
        .map(|package| (&package.id, lower_absolute_crate(package)))
        .collect::<Map<_, _>>();

    let lib = "lib".to_string();
    let proc_macro = "proc-macro".to_string();

    for node in &metadata.resolve.as_ref().expect("resolve required").nodes {
        let id = &node.id;

        let package = packages[id];
        let abs_crate = abs_crates[id].clone();

        let manifest_path = package.manifest_path.clone();

        let is_proc_macro = package
            .targets
            .iter()
            .find(|target| target.kind.contains(&proc_macro))
            .is_some();

        let src_root = package
            .targets
            .iter()
            .find(|target| target.kind.contains(&lib) || target.kind.contains(&proc_macro));
        let src_root = if let Some(src_root) = src_root {
            src_root.src_path.parent().unwrap().to_owned()
        } else {
            warn!(
                "skipping package with no lib target: {:?} {:?}",
                abs_crate, package.targets
            );
            continue;
        };

        let features = node.features.clone();

        let deps = node
            .deps
            .iter()
            .map(|dep| {
                (
                    dep.name[..].replace("-", "_").into(),
                    abs_crates[&dep.pkg].clone(),
                )
            })
            .collect();

        let cargo_source = package.source.clone();

        // TODO edition

        result.insert(
            abs_crate,
            CrateData {
                manifest_path,
                src_root,
                features,
                deps,
                cargo_source,
                is_proc_macro,
            },
        );
    }
    result
}
