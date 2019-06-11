#![feature(async_await)]

pub mod resolve;
pub mod tools;

custom_error::custom_error! { pub Error
    Io { source: std::io::Error } = "io error: {source}",
    CargoCheckFailed = "`cargo check` failed",
    Other { cause: StaticStr } = "other error: {cause}",
    ExpansionFailure { cause: StaticStr } = "macro expansion failure: {cause}",
    ProcMacro { name: String } = "cannot expand macro {name} because it is a proc-macro",
}

/// Workaround for custom_error having parser problems.
pub type StaticStr = &'static str;

pub type Result<T> = std::result::Result<T, Error>;
