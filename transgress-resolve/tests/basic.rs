use cargo_metadata::{CargoOpt, Metadata, MetadataCommand};
use failure::ResultExt;
use std::error::Error;
use std::path::Path;
use tracing::info;
use transgress_resolve as resolve;

#[test]
fn basic() -> Result<(), Box<dyn Error>> {
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
    let root = resolve::tools::get_crate(root);

    info!("root package {:?}", root);

    let crates = resolve::tools::lower_crates(&metadata);

    let transitive_deps = resolve::tools::transitive_dependencies(&root, &crates);

    for id in &transitive_deps {
        info!("transitive dep: {:?}", id);
    }

    // TODO inject libstd/alloc/core `rustc --print sysroot`
    // TODO parse libstd/alloc/core to gzipped form distributed w/ tools, parse #[since] annotations
    //     libflate = "0.1.26"

    /*
    let resolver = resolve::resolver::Resolver::new(test_crate.clone())?;

    resolver.parse_crate(resolver.root.clone())?;
    */

    Ok(())
}

// TODO: `core`, parse and resolve all items in `core`
