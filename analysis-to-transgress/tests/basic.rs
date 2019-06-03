#![feature(async_await)]

extern crate analysis_to_transgress as a2t;

#[runtime::test]
async fn basic() -> a2t::Result<()> {
    let _ = pretty_env_logger::try_init();

    a2t::tools::ensure_analysis("../test-crate".as_ref()).await?;
    let analysis = a2t::tools::fetch_analysis("../test-crate".as_ref()).await?;

    Ok(())
}
