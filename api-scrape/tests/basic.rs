#![feature(async_await)]

extern crate api_scrape as api;

#[runtime::test]
async fn basic() -> a2t::Result<()> {
    let _ = pretty_env_logger::try_init();

    api::tools::check("../test-crate".as_ref()).await?;

    Ok(())
}
