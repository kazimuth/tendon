extern crate transgress_source_scrape as scrape;

#[test]
fn basic() -> scrape::Result<()> {
    spoor::init();

    scrape::tools::check("../test-crate".as_ref())?;

    let resolver = scrape::resolve::Resolver::new("../test-crate".into())?;

    resolver.parse_crate(resolver.root.clone())?;

    Ok(())
}

#[test]
fn syn_stuff() -> scrape::Result<()> {
    println!("{:?}", syn::parse_str::<syn::Type>("bees<str>::Vec<i32>")?);
    Ok(())
}
