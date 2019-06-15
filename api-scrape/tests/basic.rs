#![feature(async_await)]

extern crate api_scrape as api;

#[runtime::test]
async fn basic() -> api::Result<()> {
    spoor::init();

    api::tools::check("../test-crate".as_ref()).await?;

    let _resolver = api::resolve::Resolver::new("../test-crate".into()).await?;

    Ok(())
}
