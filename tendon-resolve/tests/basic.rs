use cargo_metadata::{CargoOpt, MetadataCommand};
use failure::ResultExt;
use rayon::prelude::*;
use std::error::Error;
use std::path::Path;
use std::time::Instant;
use tendon_api::paths::{CrateId, AbsolutePath};
use tendon_resolve as resolve;

/*

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

    let mut crates = resolve::tools::lower_crates(&metadata);
    resolve::tools::add_rust_sources(&mut crates, &test_crate)?;

    let transitive_deps = resolve::tools::transitive_dependencies(&root, &crates);

    let mut ordered = transitive_deps.iter().collect::<Vec<_>>();
    ordered.sort();

    let mut hit = resolve::Set::default();

    let db = resolve::Db::new();

    let start = Instant::now();

    loop {
        let next = if let Some(next) = ordered.iter().find(|crate_| {
            !hit.contains(**crate_) && crates[**crate_].deps.values().all(|dep| hit.contains(dep))
        }) {
            next
        } else {
            break;
        };
        hit.insert((*next).clone());
        println!("walking {:?}", next);

        if let Err(err) = resolve::walker::walk_crate(crates.get_mut(*next).unwrap(), &db) {
            println!("crate walk error: {:?}", err);
        }
    }

    println!(
        "parse all deps (serial) elapsed time: {}ms",
        start.elapsed().as_millis()
    );

    let test_path = |s: &str| AbsolutePath::new(root.clone(), s.split("::"));

    assert!(db.modules.contains(&test_path("x")));
    assert!(db.modules.contains(&test_path("z")));

    assert!(db.symbols.contains(&test_path("x")));
    assert!(db.symbols.contains(&test_path("gen1")));
    assert!(db.symbols.contains(&test_path("gen2")));
    assert!(db.symbols.contains(&test_path("gen3")));
    assert!(db.symbols.contains(&test_path("uses_other")));

    assert!(db.types.contains(&test_path("Opaque")));
    assert!(db.types.contains(&test_path("Borrows")));
    assert!(db.types.contains(&test_path("NonOpaque")));
    assert!(db.types.contains(&test_path("PartiallyOpaque")));
    assert!(db.types.contains(&test_path("ReprC")));
    assert!(db.types.contains(&test_path("z::InMod")));
    assert!(db.types.contains(&test_path("WackyTupleStruct")));

    assert!(db.types.contains(&test_path("Expanded")));
    assert!(db.types.contains(&test_path("ExpandedAlt")));

    Ok(())
}

#[test]
#[ignore]
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

    let _unresolved = resolve::walker::walk_crate(&mut crates.remove(&core).unwrap(), &db)?;

    println!(
        "time to parse core: {}ms ({})",
        (Instant::now() - start).as_millis(),
        MODE
    );

    Ok(())
}

#[test]
#[ignore]
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

    resolve::walker::walk_crate(&mut crates.remove(&core).unwrap(), &db)?;
    resolve::walker::walk_crate(&mut crates.remove(&alloc).unwrap(), &db)?;
    resolve::walker::walk_crate(&mut crates.remove(&std).unwrap(), &db)?;

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

    crates.into_par_iter().for_each(|(_, mut crate_)| {
        let _ = resolve::walker::walk_crate(&mut crate_, &db);
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

    Ok(())
}
*/
