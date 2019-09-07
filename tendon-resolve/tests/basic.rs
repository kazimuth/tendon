use cargo_metadata::{CargoOpt, MetadataCommand};
use failure::ResultExt;
use rayon::prelude::*;
use std::error::Error;
use std::path::Path;
use std::time::Instant;
use tendon_api::paths::AbsoluteCrate;
use tendon_resolve as resolve;

#[cfg(debug_assertions)]
static MODE: &'static str = "debug";
#[cfg(not(debug_assertions))]
static MODE: &'static str = "release";

#[test]
fn walk_test_crate() -> Result<(), Box<dyn Error>> {
    spoor::init();
    let manifest_dir: &Path = env!("CARGO_MANIFEST_DIR").as_ref();
    let test_crate = manifest_dir.parent().unwrap().join("test-crate");

    resolve::tools::check(&test_crate)?;

    // todo: tell it about --release??
    println!("Collecting cargo metadata");
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

    println!("root package {:?}", root);

    let mut crates = resolve::tools::lower_crates(&metadata);
    resolve::tools::add_rust_sources(&mut crates, &test_crate)?;

    let transitive_deps = resolve::tools::transitive_dependencies(&root, &crates);

    let mut ordered = transitive_deps.iter().collect::<Vec<_>>();
    ordered.sort();

    for id in &ordered {
        println!("transitive dep: {:?}", id);
    }

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

    let core = AbsoluteCrate::new("core", "0.0.0");

    let db = resolve::Db::new();

    let start = Instant::now();

    let _unresolved =
        resolve::walker::walk_crate(&mut crates.remove(&core).unwrap(), &db)?;

    println!(
        "time to parse core: {}ms ({})",
        (Instant::now() - start).as_millis(),
        MODE
    );

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

    let core = AbsoluteCrate::new("core", "0.0.0");
    let alloc = AbsoluteCrate::new("alloc", "0.0.0");
    let std = AbsoluteCrate::new("std", "0.0.0");

    let db = resolve::Db::new();

    let start = Instant::now();

    resolve::walker::walk_crate( &mut crates.remove(&core).unwrap(), &db)?;
    resolve::walker::walk_crate( &mut crates.remove(&alloc).unwrap(), &db)?;
    resolve::walker::walk_crate( &mut crates.remove(&std).unwrap(), &db)?;

    println!(
        "time to parse stdlib: {}ms ({})",
        (Instant::now() - start).as_millis(),
        MODE
    );
    println!(
        "found in stdlib: {} types, {} symbols, {} modules",
        db.types.len(),
        db.symbols.len(),
        db.modules.len()
    );

    Ok(())
}

#[ignore]
#[test]
fn walk_repo_deps() -> Result<(), Box<dyn Error>> {
    spoor::init();
    let manifest_dir: &Path = env!("CARGO_MANIFEST_DIR").as_ref();
    let test_crate = manifest_dir.parent().unwrap().join("test-crate");

    resolve::tools::check(&test_crate)?;

    println!("Collecting cargo metadata");
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

    println!("root package {:?}", root);

    let mut crates = resolve::tools::lower_crates(&metadata);
    resolve::tools::add_rust_sources(&mut crates, &test_crate)?;

    let db = resolve::Db::new();

    let start = Instant::now();

    crates.into_par_iter().for_each(|(dep, mut crate_)| {
        let _ = resolve::walker::walk_crate( &mut crate_, &db);
    });
    println!(
        "time to parse all repo deps: {}ms",
        (Instant::now() - start).as_millis()
    );
    println!(
        "found in all repo deps: {} types, {} symbols, {} modules",
        db.types.len(),
        db.symbols.len(),
        db.modules.len()
    );

    // TODO: add similar to ^ output in console stdout
    // TODO: measure # of relevant parse / lowering failures

    Ok(())
}
