use std::future::Future;

/// Start another future
pub fn spawn<F: Future + Send + 'static>(f: F) -> impl Future<Output = <F as Future>::Output> {}
