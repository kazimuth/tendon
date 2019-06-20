//! Helpers for tokio-trace and std futures.

use std::future::Future;
use std::pin::Pin;
use std::task;

use tokio_trace::Span;

/// Init a basic env-logger based tracing system.
pub fn init() {
    let _ = tokio_trace_env_logger::try_init();
    let subscriber = tokio_trace_fmt::FmtSubscriber::builder()
        .on_event(tokio_trace_fmt::default::fmt_verbose)
        .finish();
    let _ = tokio_trace::subscriber::set_global_default(subscriber);
}
