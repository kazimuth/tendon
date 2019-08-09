use std::io;
use std::path::Path;
use std::process::Command;
use tracing::info;

/// Run `cargo check` on target project to ensure well-formed input + dependencies.
pub fn check(path: &Path) -> io::Result<()> {
    info!("ensuring well-formed input");

    info!("$ cd {} && cargo check", path.display());
    let path_ = path.to_owned();
    let status = Command::new("cargo")
        .args(&["check"])
        .current_dir(path_)
        .status()?;

    if !status.success() {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "failed to run cargo check",
        ))
    } else {
        Ok(())
    }
}
