use std::error::Error;

#[test]
fn list_api() -> Result<(), Box<dyn Error>> {
    let _ = pretty_env_logger::try_init();

    let _api = rustdoc_scrape::extract_for_crate("../test-crate")?;

    Ok(())
}
