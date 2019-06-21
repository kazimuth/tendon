extern crate api_scrape as api;

#[test]
fn basic() -> api::Result<()> {
    spoor::init();

    api::tools::check("../test-crate".as_ref())?;

    let resolver = api::resolve::Resolver::new("../test-crate".into())?;

    resolver.parse_crate(resolver.root.clone())?;

    Ok(())
}
