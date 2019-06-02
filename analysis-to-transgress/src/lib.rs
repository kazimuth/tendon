pub mod tools;

custom_error::custom_error! { pub Error
    Io { source: std::io::Error } = "io error: {source}",
    CargoCheckFailed = "`cargo check` failed",
    RlsFailed = "`rls` failed",
    Other { cause: StaticStr } = "other error: {cause}",
    AnalysisError { source: rls_analysis::AError } = "rls-analysis error: {source}",
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
