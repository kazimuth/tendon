#![feature(async_await)]

pub mod sigbuilder;
pub mod db;
pub mod inspect;
pub mod tools;

custom_error::custom_error! { pub Error
    Io { source: std::io::Error } = "io error: {source}",
    CantResolveSysroot = "can't find sysroot for save-analysis: need to specify SYSROOT or RUSTC env vars, or rustc must be in PATH",
    CargoCheckFailed = "`cargo check` failed",
    RlsFailed = "`rls` failed",
    RlsTimeout = "`rls` timeout",
    // TODO: refactor to tuple
    Other { cause: StaticStr } = "other error: {cause}",
    Json { source: serde_json::Error } = "serde_json error: {source}",
    MissingSaveAnalysis { dir: String } = "missing save analysis, expected in folder: {dir}",
    Cancelled { source: futures::channel::oneshot::Canceled } = "{source}",
    FromUtf8 { source: std::string::FromUtf8Error } = "{source}",
    Poison = "poison",
}

/// Workaround for custom_error having parser problems.
pub type StaticStr = &'static str;

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
