//! Runtime for the blocking thread pool.

use log::info;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::Duration;

lazy_static::lazy_static! {
    static ref RUNTIME: Runtime = init_runtime();
}

// Correctness proof:
// We need to guarantee that every future whose closure returns will eventually be polled.
// Events:
// - P1: result is set from thread pool
// - P2: waker is called from thread pool, if extant
// - C1: waker is set from executor
// - C2: result is returned from executor, if extant [end state]
//
// P1 -> P2, C1 -> C2
// assume executor fulfills contract of wakers (if waker is called, future will be polled later)
//
// requirement: P1 precedes C2
//
// Possible interleavings:
// P1 P2 C1 C2*
// P1 C1 P2* C2*
// P1 C1 C2* P2*
// C1 P1 C2* P2*
// C1 C2 P1 P2* C2*
//
// all are valid.

/// Enqueue a blocking work item to be performed some time in the future, on the blocking thread pool.
pub fn unblock<F, T>(f: F) -> impl Future<Output = T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let inner = Arc::new(FutureInner {
        waker: OnceCell::new(),
        result: OnceCell::new(),
    });
    let inner_clone = inner.clone();

    let op = Box::new(move || {
        let result = f();
        let raw = Box::into_raw(Box::new(result)) as *mut ();

        // EVENT P1
        if !inner.result.set(SendPtr(raw)).is_ok() {
            panic!("invariants violated");
        }

        // EVENT P2
        if let Some(waker) = inner.waker.get() {
            waker.wake_by_ref();
        }
    });

    RUNTIME.injector.lock().push(op);

    UnblockFuture {
        returned: false,
        inner: inner_clone,
        phantom: PhantomData,
    }
}

struct FutureInner {
    waker: OnceCell<Waker>,
    result: OnceCell<SendPtr>,
}
struct SendPtr(*mut ());
unsafe impl Send for SendPtr {}
// TODO: actually sync? why does oncecell require this?
unsafe impl Sync for SendPtr {}

struct UnblockFuture<T> {
    // required to prevent accidental aliasing if poll() is called after Ready() is returned
    returned: bool,
    inner: Arc<FutureInner>,
    phantom: PhantomData<T>,
}

impl<T> Future for UnblockFuture<T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if self.returned {
            panic!("UnblockFuture polled after returning")
        }
        {
            // EVENT C1
            self.inner.waker.get_or_init(|| cx.waker().clone());

            // EVENT C2
            if let Some(result) = self.inner.result.get() {
                let boxed = unsafe { Box::from_raw((result.0) as *mut T) };

                unsafe {
                    self.get_unchecked_mut().returned = true;
                }

                Poll::Ready(*boxed)
            } else {
                Poll::Pending
            }
        }
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
                let mut rest = 1; // sleep time, ms; exponential backoff

                loop {
                    let next = { RUNTIME.injector.lock().pop() };
                    if let Some(next) = next {
                        rest = 1;
                        next();
                    } else {
                        rest = (rest * 2).min(1000);
                        thread::sleep(Duration::from_millis(rest));
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

    use std::ptr;
    use std::task::{RawWaker, RawWakerVTable};
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
        let count = 1000u32;
        let mut ops: Vec<_> = (0..count).map(move |v| unblock(move || v)).collect();
        for (i, op) in ops.drain(..).enumerate() {
            assert_eq!(test_await(op), i as u32);
        }
        println!(
            "stress elapsed: {:?} ({:?} per item)",
            start.elapsed(),
            start.elapsed() / count
        );
    }
}
