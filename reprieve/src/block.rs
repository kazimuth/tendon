//! Convert blocking code into futures.

use log::info;
use parking_lot::Mutex;
use std::future::Future;
use std::thread;

use crate::once::once_future;

lazy_static::lazy_static! {
    static ref RUNTIME: Runtime = init_runtime();
}

/// Enqueue a blocking work item to be performed some time in the future, on a
/// global thread pool for blocking work.
///
/// ```no_run
/// # #![feature(async_await)]
/// # use std::future::Future;
/// # use std::path::PathBuf;
/// # use std::fs::File;
/// # use std::io::{self, Read};
/// # use std::thread;
/// # use std::time::Duration;
/// /// most basic example: do something blocking without blocking the executor thread
/// /// (note: there's no reason to actually use this function, lmao)
/// async fn nonblocking_sleep(duration: Duration) {
///     reprieve::unblock(move || thread::sleep(duration)).await
/// }
///
/// /// Find the name of the current rust package.
/// async fn package_name() -> io::Result<String> {
///     let toml_path = PathBuf::from("Cargo.toml");
///
///     // create a future, not blocking the executor
///     let cargo_toml = reprieve::unblock(move || -> io::Result<String> {
///         let mut file = File::open(toml_path)?;
///         let mut result = String::new();
///         file.read_to_string(&mut result)?;
///         Ok(result)
///     });
///
///     // ... do some work ...
///
///     let cargo_toml = cargo_toml.await?;
///     
///     let name = cargo_toml.lines().find(|line| line.starts_with("name")).unwrap_or("unknown");
///     Ok(name.into())
/// }
///
pub fn unblock<F, T>(f: F) -> impl Future<Output = T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (sender, future) = once_future();

    let op = Box::new(move || {
        let result = f();
        sender.set(result);
    });

    RUNTIME.injector.lock().push(op);

    future
}

fn init_runtime() -> Runtime {
    info!("starting reprieve blocking thread pool");
    for i in 0..num_cpus::get() {
        let name = format!(
            "reprieve {} blocking worker {}",
            env!("CARGO_PKG_VERSION"),
            i
        );
        info!("starting thread `{}`", &name);
        thread::Builder::new()
            .name(name)
            .spawn(|| {
                let mut backoff = crate::backoff::Backoff::new(1000);

                loop {
                    let next = { RUNTIME.injector.lock().pop() };
                    if let Some(next) = next {
                        next();
                        backoff.reset();
                    } else {
                        backoff.wait();
                    }
                }
            })
            .expect("failed to start runtime");
    }
    Runtime {
        injector: Mutex::new(Vec::new()),
    }
}

struct Runtime {
    injector: Mutex<Vec<Box<dyn FnOnce() + Send>>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::pin::Pin;
    use std::ptr;
    use std::task::{Context, Poll, Waker};
    use std::task::{RawWaker, RawWakerVTable};
    use std::time::Duration;
    use std::time::Instant;

    lazy_static::lazy_static! {
        static ref VTABLE: RawWakerVTable = RawWakerVTable::new(
            |_| RawWaker::new(ptr::null(), &*VTABLE),
            |_| {},
            |_| {},
            |_| {}
        );
    }

    fn test_await<T, F: Future<Output = T>>(mut f: F) -> T {
        let waker = unsafe { Waker::from_raw(RawWaker::new(ptr::null(), &*VTABLE)) };
        let mut context = Context::from_waker(&waker);

        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(1000) {
            let pinned = unsafe { Pin::new_unchecked(&mut f) };
            match pinned.poll(&mut context) {
                Poll::Pending => thread::sleep(Duration::from_millis(1)),
                Poll::Ready(result) => {
                    return result;
                }
            }
        }
        panic!("timeout: 1s");
    }

    #[test]
    fn basic() {
        let _ = pretty_env_logger::try_init();

        let op = unblock(|| 0);
        let result = test_await(op);
        assert_eq!(result, 0);
    }

    #[test]
    fn delayed() {
        let _ = pretty_env_logger::try_init();

        let op = unblock(|| {
            thread::sleep(Duration::from_millis(20));
            0
        });
        let result = test_await(op);
        assert_eq!(result, 0);
    }

    #[test]
    fn stress() {
        let _ = pretty_env_logger::try_init();

        let start = Instant::now();
        let count = 10000u32;
        let mut ops: Vec<_> = (0..count).map(move |v| unblock(move || v)).collect();
        for (i, op) in ops.drain(..).enumerate() {
            assert_eq!(test_await(op), i as u32);
        }
        println!(
            "block stress elapsed: {:?} ({:?} per item)",
            start.elapsed(),
            start.elapsed() / count
        );
    }
}
