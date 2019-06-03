#![feature(async_await)]

//! Convert blocking code into futures.
//!
//! ## Example with std::io
//!
//! ```no_run
//! # #![feature(async_await)]
//! use std::{
//!     io, fs, path::Path,
//! };
//!
//! // declare the error type you want to use in this module
//! // alternatively, `use reprieve::unblock;`
//! reprieve::use_error!(io::Error);
//!
//! async fn read_to_string<P: AsRef<Path>>(path: P) -> io::Result<String> {
//!     // convert blocking code to a future
//!     let result = reprieve::unblocked! {
//!         let path = path.as_ref().to_owned();
//!         fs::read_to_string(path)?
//!     };
//!     result.await
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
/// ```
pub fn unblock<F, T>(input: F) -> impl Future<Output = T>
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
        future.await.expect("runtime dropped channel, should never happen")
    }
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

/// Set the error type to use for this module.
///
/// Declares a local function `unblock` like `reprieve::unblock` but that requires
/// its input return Result<T, $err>.
#[macro_export]
macro_rules! use_error {
    ($err:ty) => {
        fn unblock<F, T>(
            input: F,
        ) -> impl std::future::Future<Output = std::result::Result<T, $err>>
        where
            T: Send + 'static,
            F: FnOnce() -> std::result::Result<T, $err> + Send + 'static,
        {
            $crate::unblock(input)
        }
    };
}

/// Unblock a bit of blocking code by running it off the executor.
///
/// Calls whatever `unblock` is in scope; either `use reprieve::unblock;` or
/// `reprieve::set_error(ErrorType);`.
///
/// ```no_run
/// # #![feature(async_await)]
/// use std::{fs, io, path::Path};
/// use reprieve::unblock;
/// 
/// async fn read_to_string(path: &Path) -> io::Result<String> {
///     (reprieve::unblocked! {
///         // you can add 'let' bindings to make things Send
///         let path = path.to_owned(); 
///         // then finish with an expression:
///         fs::read_to_string(path)?
///      }).await
/// }
///
/// ```
/// 
#[macro_export]
macro_rules! unblocked {
    ($(let $i:ident = $b:expr;)* $ex:expr) => {
        {
            $(let $i = $b;)*
            unblock(move || {
                Ok($ex)
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;
    use futures_timer::FutureExt;
    use std::time::{Duration, Instant};
    use std::thread;

    crate::use_error!(std::io::Error);

    #[test]
    fn basic() {
        let _ = pretty_env_logger::try_init();

        let op = crate::unblocked! {
            37
        }.timeout(Duration::from_secs(1));

        let result = block_on(op).unwrap();

        assert_eq!(result, 37);
    }

    #[test]
    fn delayed() {
        let _ = pretty_env_logger::try_init();

        let op = unblocked!{
            let _t = thread::sleep(Duration::from_millis(20));
            0
        };
        let result = block_on(op).unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn stress() {
        let _ = pretty_env_logger::try_init();

        let start = Instant::now();
        let count = 10000u32;
        let mut ops: Vec<_> = (0..count).map(move |v| crate::unblock(move || v)).collect();
        for (i, op) in ops.drain(..).enumerate() {
            assert_eq!(block_on(op), i as u32);
        }
        println!(
            "block stress elapsed: {:?} ({:?} per item)",
            start.elapsed(),
            start.elapsed() / count
        );
    }
}
