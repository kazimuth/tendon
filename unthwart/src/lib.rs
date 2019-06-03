#![feature(async_await, await_macro)]

//! Don't block (thwart) your event loop.
//!
//! ## Example with std::io
//!
//! ```no_run
//! # #![feature(async_await)]
//! use std::{
//!     io, fs, path::PathBuf,
//! };
//!
//! // declare the error type you want to use in this module
//! // alternatively, just use `unthwart` directly
//! type Error = std::io::Error;
//!
//! async fn read_to_string(path: PathBuf) -> io::Result<String> {
//!     // convert blocking code to a future
//!     unthwart::unthwarted! {
//!         fs::read_to_string(&path)
//!     }
//! }
//! ```

mod backoff;

use futures::channel::oneshot;
use log::info;
use parking_lot::Mutex;
use std::future::Future;
use std::thread;

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
/// /// Find the name of the current rust package.
/// async fn package_name() -> io::Result<String> {
///     let toml_path = PathBuf::from("Cargo.toml");
///
///     // create a future, not blocking the executor
///     // note: future must be Send + 'static, which means you
///     // should make all the inputs owned and move them into the closure
///     let cargo_toml = unthwart::unthwart(move || -> io::Result<String> {
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
/// ```
pub fn unthwart<F, T>(input: F) -> impl Future<Output = T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (sender, future) = oneshot::channel();

    let op = Box::new(move || {
        let result = input();
        let _ = sender.send(result);
    });

    RUNTIME.injector.lock().push(op);

    async {
        future
            .await
            .expect("runtime dropped channel, should never happen")
    }
}

/// Unblock a bit of blocking code by running it off the executor.
///
/// Returns a Result<T, Error>, where Error is whatever Error is in scope.
///
/// ```no_run
/// # #![feature(async_await)]
/// use std::{fs, io::{self, Error}, path::Path};
/// use unthwart::unthwart;
///
/// async fn read_to_string(path: &Path) -> io::Result<String> {
///     let path = path.to_owned();
///     unthwart::unthwarted! {
///         fs::read_to_string(&path)
///     }
/// }
/// ```
///
#[macro_export]
macro_rules! unthwarted {
    ($($op:tt)+) => ({
        let f = move || -> Result<_, Error> {
            Ok($crate::as_expr!({$($op)*}))
        };
        $crate::unthwart(f).await?
    })
}

#[macro_export]
macro_rules! unthwarted_better {
    ($($op:tt)+) => ({
        use parking_lot::Mutex;
        use std::sync::Arc;
        struct Dropper(Arc<Mutex<bool>>);
        impl Drop for Dropper {
            fn drop(&mut self) {
                *self.0.lock() = false;
            }
        }
        let accessible = Arc::new(Mutex::new(true));
        let _root = Dropper(accessible.clone());

        struct NoReallySafeIPromise<T>(*mut T);
        unsafe impl<T> Send for NoReallySafeIPromise<T> {}

        fn ensure_sync<T: Sync>(t: &T) {}

        let mut f = || -> Result<_, Error> {Ok($crate::as_expr!({$($op)*}))};
        ensure_sync(&f);

        let addr = NoReallySafeIPromise(&mut f);
        let z = move || {
            if *accessible.lock() {
                Some(unsafe { (*addr.0)() })
            } else {
                None
            }
        };

        let result: Result<_, Error> = $crate::unthwart(z).await.expect("unreachable")?;
        result
    })
}

/// The same as `unthwarted`, but doesn't coerce errors.
#[macro_export]
macro_rules! unthwarted_ {
    ($($op:tt)+) => {
        $crate::unthwart(move || $crate::as_expr!({$($op)*}))
    }
}
#[macro_export]
macro_rules! as_expr {
    ($e:expr) => {
        $e
    };
}

fn init_runtime() -> Runtime {
    info!("starting unthwart blocking thread pool");
    for i in 0..num_cpus::get() {
        let name = format!(
            "unthwart {} blocking worker {}",
            env!("CARGO_PKG_VERSION"),
            i
        );
        info!("starting thread `{}`", &name);
        thread::Builder::new()
            .name(name)
            .spawn(|| {
                let mut backoff = backoff::Backoff::new(1000);

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
    use futures::executor::block_on;
    use futures_timer::FutureExt;
    use std::thread;
    use std::time::{Duration, Instant};

    use std::io::Error;

    #[test]
    fn basic() {
        let _ = pretty_env_logger::try_init();

        let op = async {
            Result::<_, Error>::Ok(crate::unthwarted! {
                37
            })
        }
            .timeout(Duration::from_secs(1));

        let result = block_on(op).unwrap();

        assert_eq!(result, 37);
    }

    #[test]
    fn delayed() {
        let _ = pretty_env_logger::try_init();

        let op = async {
            unthwarted! {
                thread::sleep(Duration::from_millis(20));
                Result::<_, Error>::Ok(69u32)
            }
        };
        let before = Instant::now();
        let result = block_on(op).unwrap();
        assert_eq!(result, 69);
        assert!(before.elapsed() > Duration::from_millis(10));
    }

    #[test]
    fn stress() {
        let _ = pretty_env_logger::try_init();

        let start = Instant::now();
        let count = 10000u32;
        let mut ops: Vec<_> = (0..count).map(move |v| unthwarted_!(v)).collect();
        for (i, op) in ops.drain(..).enumerate() {
            assert_eq!(block_on(op), i as u32);
        }
        println!(
            "block stress elapsed: {:?} ({:?} per item)",
            start.elapsed(),
            start.elapsed() / count
        );
    }

    use std::{fs, io, path::Path};

    async fn read_to_string(path: &Path) -> io::Result<String> {
        crate::unthwarted_better! {
            fs::read_to_string(&path)
        }
    }
}
