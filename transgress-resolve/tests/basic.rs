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

    let Metadata {
        mut packages,
        resolve: meta_resolve,
        workspace_root: _workspace_root,
        ..
    } = metadata;
    let package_map: resolve::Map<_, _> = packages
        .iter()
        .map(|package| (package.id.clone(), package.clone()))
        .collect();
    let mut meta_resolve = meta_resolve.unwrap();
    let root = meta_resolve.root.unwrap();
    info!("root package {:?}", root.repr);
    let nodes: resolve::Map<_, _> = meta_resolve
        .nodes
        .iter()
        .map(|node| (&node.id, node))
        .collect();

    let sorted = resolve::tools::topo_sort_nodes(&meta_resolve.nodes);

    let mut visited = resolve::Set::default();

    for id in &sorted {
        info!("next: {:?}", resolve::tools::get_crate(&package_map[id]));
        visited.insert(id);
        for dep in &nodes[id].dependencies {
            assert!(visited.contains(dep));
        }
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
