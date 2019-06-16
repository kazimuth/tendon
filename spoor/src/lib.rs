//! Helpers for tokio-trace and std futures.

#![feature(async_await)]

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

/// Trace a future.
pub fn trace<F: Future>(span: tokio_trace::Span, future: F) -> impl Future<Output = F::Output> {
    TraceFuture { future, span }
}

struct TraceFuture<F: Future> {
    future: F,
    span: Span,
}
impl<F: Future> Future for TraceFuture<F> {
    type Output = F::Output;
    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut task::Context) -> task::Poll<F::Output> {
        let &mut TraceFuture {
            ref span,
            ref mut future,
        } = unsafe { Pin::get_unchecked_mut(self) };
        let _guard = span.enter();
        unsafe { Pin::new_unchecked(future) }.poll(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_trace::{info_span, span::*, Event, Metadata};

    #[test]
    fn polls_correctly() {
        static mut ENTRIES: usize = 0;
        struct TestSubscriber;
        impl tokio_trace::subscriber::Subscriber for TestSubscriber {
            fn enabled(&self, _: &Metadata) -> bool {
                true
            }
            fn new_span(&self, _: &Attributes) -> Id {
                Id::from_u64(1)
            }
            fn record(&self, _: &Id, _: &Record) {}
            fn record_follows_from(&self, _: &Id, _: &Id) {}
            fn event(&self, _: &Event) {}
            fn enter(&self, _: &Id) {
                unsafe {
                    ENTRIES += 1;
                }
            }
            fn exit(&self, _: &Id) {}
        }

        let result = tokio_trace::subscriber::with_default(TestSubscriber, || {
            let f = trace(info_span!("test span"), async { 37u8 });
            futures::executor::block_on(f)
        });
        assert_eq!(result, 37u8);
        assert_eq!(unsafe { ENTRIES }, 1);
    }
}
