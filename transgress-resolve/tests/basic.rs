use transgress_resolve as resolve;

use cargo_metadata::{Metadata, MetadataCommand, CargoOpt};

use std::path::Path;

#[test]
fn basic() -> scrape::Result<()> {
    spoor::init();
    let source_scrape: &Path = env!("CARGO_MANIFEST_DIR").as_ref();
    let test_crate = source_scrape.parent().unwrap().join("test-crate");

    scrape::tools::check(&test_crate)?;

    let project = project_root.clone();
    info!("Collecting cargo metadata");
    let metadata = MetadataCommand::new()
        .current_dir(&project)
        .manifest_path(&project.join("Cargo.toml"))
        .features(CargoOpt::AllFeatures)
        .exec()?;

    let Metadata {
        mut packages,
        resolve,
        workspace_root,
        ..
    } = metadata;
    let packages: resolve::Map<_, _> = packages
        .drain(..)
        .map(|package| (package.id.clone(), package))
        .collect();
    let mut resolve = resolve.ok_or(Error::ResolveFailed)?;
    let root = resolve.root.ok_or(Error::ResolveFailed)?;
    let resolve = resolve
        .nodes
        .drain(..)
        .map(|node| (node.id.clone(), node))
        .collect();

    info!("root package {:?}", root.repr);

    let resolver = resolve::resolver::Resolver::new(test_crate.clone())?;

    resolver.parse_crate(resolver.root.clone())?;

    Ok(())
}

#[test]
fn syn_stuff() -> scrape::Result<()> {
    println!("{:?}", syn::parse_str::<syn::Type>("bees<str>::Vec<i32>")?);
    Ok(())
}
