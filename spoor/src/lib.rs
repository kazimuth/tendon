//! Helpers for tracing and std futures.



/// Init a basic env-logger based tracing system.
pub fn init() {
    let _ = tracing_env_logger::try_init();
    let subscriber = tracing_fmt::FmtSubscriber::builder()
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
}
