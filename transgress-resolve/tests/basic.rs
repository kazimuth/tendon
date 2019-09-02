use cargo_metadata::{CargoOpt, Metadata, MetadataCommand};
use failure::ResultExt;
use std::error::Error;
use std::path::Path;
use tracing::info;
use transgress_resolve as resolve;
use transgress_api::paths::AbsoluteCrate;
use std::time::Instant;
use rayon::prelude::*;

#[test]
fn walk_test_crate() -> Result<(), Box<dyn Error>> {
    spoor::init();
    let manifest_dir: &Path = env!("CARGO_MANIFEST_DIR").as_ref();
    let test_crate = manifest_dir.parent().unwrap().join("test-crate");

    resolve::tools::check(&test_crate)?;

    info!("Collecting cargo metadata");
    let metadata = MetadataCommand::new()
        .current_dir(&test_crate)
        .manifest_path(&test_crate.join("Cargo.toml"))
        .features(CargoOpt::AllFeatures)
        .exec()
        .compat()?;

    let root = metadata.resolve.as_ref().unwrap().root.as_ref().unwrap();
    let root = metadata
        .packages
        .iter()
        .find(|package| &package.id == root)
        .unwrap();
    let root = resolve::tools::lower_absolute_crate(root);

    info!("root package {:?}", root);

    let mut crates = resolve::tools::lower_crates(&metadata);
    resolve::tools::add_rust_sources(&mut crates, &test_crate)?;

    let transitive_deps = resolve::tools::transitive_dependencies(&root, &crates);

    let mut ordered = transitive_deps.iter().collect::<Vec<_>>();
    ordered.sort();

    for id in &ordered {
        info!("transitive dep: {:?}", id);
    }

    //let db = resolve::resolver::Db::new();

    //let start = Instant::now();

    //for dep in &ordered {
    //    resolve::resolver::walker::walk_crate(
    //        (*dep).clone(),
    //        &crates[dep],
    //        &db,
    //    )?;
    //}
    //println!("time to parse all test-crate deps: {}ms", (Instant::now() - start).as_millis());

    Ok(())
}

#[test]
fn walk_core() -> Result<(), Box<dyn Error>> {
    spoor::init();
    let manifest_dir: &Path = env!("CARGO_MANIFEST_DIR").as_ref();
    let test_crate = manifest_dir.parent().unwrap().join("test-crate");

    resolve::tools::check(&test_crate)?;

    let mut crates = resolve::Map::default();
    resolve::tools::add_rust_sources(&mut crates, &test_crate)?;

    let core = AbsoluteCrate {
        name: "core".into(),
        version: "0.0.0".into()
    };

    let db = resolve::resolver::Db::new();

    let start = Instant::now();

    let unresolved = resolve::resolver::walker::walk_crate(
        core.clone(),
        &crates[&core],
        &db,
    )?;

    println!("time to parse core: {}ms", (Instant::now() - start).as_millis());

    Ok(())
}

#[test]
fn walk_stdlib() -> Result<(), Box<dyn Error>> {
    spoor::init();
    let manifest_dir: &Path = env!("CARGO_MANIFEST_DIR").as_ref();
    let test_crate = manifest_dir.parent().unwrap().join("test-crate");

    resolve::tools::check(&test_crate)?;

    let mut crates = resolve::Map::default();
    resolve::tools::add_rust_sources(&mut crates, &test_crate)?;

    let core = AbsoluteCrate {
        name: "core".into(),
        version: "0.0.0".into()
    };
    let alloc = AbsoluteCrate {
        name: "alloc".into(),
        version: "0.0.0".into()
    };
    let std = AbsoluteCrate {
        name: "std".into(),
        version: "0.0.0".into()
    };

    let db = resolve::resolver::Db::new();

    let start = Instant::now();

    resolve::resolver::walker::walk_crate(
        core.clone(),
        &crates[&core],
        &db,
    )?;
    resolve::resolver::walker::walk_crate(
        alloc.clone(),
        &crates[&alloc],
        &db,
    )?;
    resolve::resolver::walker::walk_crate(
        std.clone(),
        &crates[&std],
        &db,
    )?;

    println!("time to parse stdlib: {}ms", (Instant::now() - start).as_millis());

    Ok(())
}

#[ignore]
#[test]
fn walk_repo_deps() -> Result<(), Box<dyn Error>> {
    spoor::init();
    let manifest_dir: &Path = env!("CARGO_MANIFEST_DIR").as_ref();
    let test_crate = manifest_dir.parent().unwrap().join("test-crate");

    resolve::tools::check(&test_crate)?;

    info!("Collecting cargo metadata");
    let metadata = MetadataCommand::new()
        .current_dir(&test_crate)
        .manifest_path(&test_crate.join("Cargo.toml"))
        .features(CargoOpt::AllFeatures)
        .exec()
        .compat()?;

    let root = metadata.resolve.as_ref().unwrap().root.as_ref().unwrap();
    let root = metadata
        .packages
        .iter()
        .find(|package| &package.id == root)
        .unwrap();
    let root = resolve::tools::lower_absolute_crate(root);

    info!("root package {:?}", root);

    let mut crates = resolve::tools::lower_crates(&metadata);
    resolve::tools::add_rust_sources(&mut crates, &test_crate)?;

    let mut all = crates.keys().collect::<Vec<_>>();
    all.sort();

    let db = resolve::resolver::Db::new();

    let start = Instant::now();

    all.par_iter().for_each(|dep| {
        let _ = resolve::resolver::walker::walk_crate(
            (*dep).clone(),
            &crates[dep],
            &db,
        );
    });
    println!("time to parse all repo deps: {}ms", (Instant::now() - start).as_millis());

    Ok(())
}
