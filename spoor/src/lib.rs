//! Helpers for tracing and std futures.

/// Init a basic env-logger based tracing system.
pub fn init() {
    let _ = tracing_subscriber::fmt::try_init();
    let subscriber = tracing_subscriber::fmt::Subscriber::builder().finish();
    let _ = tracing::dispatcher::set_global_default(tracing::Dispatch::new(subscriber));
}
