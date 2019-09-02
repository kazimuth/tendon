use cargo_metadata::{CargoOpt, Metadata, MetadataCommand};
use failure::ResultExt;
use std::error::Error;
use std::path::Path;
use tracing::info;
use transgress_resolve as resolve;
use transgress_api::paths::AbsoluteCrate;

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

    for id in ordered {
        info!("transitive dep: {:?}", id);
    }

    Ok(())
}

// TODO: `core`, parse and resolve all items in `core`

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

    // TODO inject libstd/alloc/core `rustc --print sysroot`
    let unresolved = resolve::resolver::walker::walk_crate(
        core.clone(),
        &crates[&core],
        &db,
    )?;

    Ok(())
}

