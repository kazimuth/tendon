use std::path::Path;
use std::process::Command;

pub type Error = Box<dyn std::error::Error>;

pub struct Api {
    pub items: Vec<String>,
}

pub fn extract_for_crate<P: AsRef<Path>>(dir: P) -> Result<Api, Error> {
    let dir = dir.as_ref();
    // run rustdoc
    let status = Command::new("cargo")
        .args(&["doc"])
        .current_dir(dir)
        .status()?;
    assert!(status.success());

    Ok(Api { items: vec![] })
}
