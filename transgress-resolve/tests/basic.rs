use transgress_resolve as resolve;
use cargo_metadata::{Metadata, MetadataCommand, CargoOpt, Node};
use std::path::Path;
use tracing::info;

#[test]
fn basic() -> resolve::Result<()> {
    spoor::init();
    let manifest_dir: &Path = env!("CARGO_MANIFEST_DIR").as_ref();
    let test_crate = manifest_dir.parent().unwrap().join("test-crate");

    resolve::tools::check(&test_crate)?;

    info!("Collecting cargo metadata");
    let metadata = MetadataCommand::new()
        .current_dir(&test_crate)
        .manifest_path(&test_crate.join("Cargo.toml"))
        .features(CargoOpt::AllFeatures)
        .exec()?;

    let Metadata {
        mut packages,
        resolve: meta_resolve,
        workspace_root,
        ..
    } = metadata;
    let packages: resolve::Map<_, _> = packages
        .drain(..)
        .map(|package| (package.id.clone(), package))
        .collect();
    let mut meta_resolve = meta_resolve.ok_or(resolve::Error::ResolveFailed)?;
    let root = meta_resolve.root.ok_or(resolve::Error::ResolveFailed)?;
    let meta_resolve: resolve::Map<_,_> = meta_resolve
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

#[test]
fn syn_stuff() -> resolve::Result<()> {
    println!("{:?}", syn::parse_str::<syn::Type>("bees<str>::Vec<i32>")?);
    Ok(())
}
