use transgress_resolve as resolve;
use cargo_metadata::{Metadata, MetadataCommand, CargoOpt};
use std::path::Path;
use tracing::info;
use std::error::Error;
use failure::ResultExt;

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
        .exec().compat()?;

    let Metadata {
        mut packages,
        resolve: meta_resolve,
        workspace_root: _workspace_root,
        ..
    } = metadata;
    let _packages: resolve::Map<_, _> = packages
        .drain(..)
        .map(|package| (package.id.clone(), package))
        .collect();
    let mut meta_resolve = meta_resolve.unwrap();
    let root = meta_resolve.root.unwrap();
    let _meta_resolve: resolve::Map<_,_> = meta_resolve
        .nodes
        .drain(..)
        .map(|node| (node.id.clone(), node))
        .collect();

    info!("root package {:?}", root.repr);

    /*
    let resolver = resolve::resolver::Resolver::new(test_crate.clone())?;

    resolver.parse_crate(resolver.root.clone())?;
    */

    Ok(())
}
