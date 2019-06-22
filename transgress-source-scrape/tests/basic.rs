extern crate transgress_source_scrape as scrape;

#[test]
fn basic() -> scrape::Result<()> {
    spoor::init();

    scrape::tools::check("../test-crate".as_ref())?;

    let resolver = scrape::resolve::Resolver::new("../test-crate".into())?;

    resolver.parse_crate(resolver.root.clone())?;

    Ok(())
}
