#![feature(async_await)]

extern crate api_scrape as api;

#[runtime::test]
async fn basic() -> api::Result<()> {
    let _ = pretty_env_logger::try_init();

    api::tools::check("../test-crate".as_ref()).await?;

    let resolver = api::resolve::Resolver::new("../test-crate".into()).await?;

    Ok(())
}
