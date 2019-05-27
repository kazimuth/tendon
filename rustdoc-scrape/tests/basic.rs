use std::error::Error;

#[test]
fn list_api() -> Result<(), Box<dyn Error>> {
    let api = rustdoc_scrape::extract_for_crate("../test-crate")?;

    Ok(())
}
