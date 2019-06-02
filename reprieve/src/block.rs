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

/// Enqueue a blocking work item to be performed some time in the future, on the blocking thread pool.
pub fn later<F, T>(f: F) -> impl Future<Output = T>
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
        if !inner.result.set(SendPtr(raw)).is_ok() {
            panic!("invariants violated");
        }

        if let Some(waker) = inner.waker.get() {
            waker.wake_by_ref();
        }
    });

    RUNTIME.injector.lock().push(op);

    BlockFuture(inner_clone, PhantomData)
}

struct FutureInner {
    waker: OnceCell<Waker>,
    result: OnceCell<SendPtr>,
}
struct SendPtr(*mut ());
unsafe impl Send for SendPtr {}
unsafe impl Sync for SendPtr {}

struct BlockFuture<T>(Arc<FutureInner>, PhantomData<T>);

impl<T> Future for BlockFuture<T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        {
            self.0.waker.get_or_init(|| cx.waker().clone());

            if let Some(result) = self.0.result.get() {
                let boxed = unsafe { Box::from_raw((result.0) as *mut T) };

                Poll::Ready(*boxed)
            } else {
                Poll::Pending
            }
        }
    }
}

fn init_runtime() -> Runtime {
    for i in 0..num_cpus::get() {
        thread::Builder::new()
            .name(format!(
                "reprieve {} blocking worker {}",
                env!("CARGO_PKG_VERSION"),
                i
            ))
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
        let op = later(|| 0);
        let result = test_await(op);
        assert_eq!(result, 0);
    }

    #[test]
    fn delayed() {
        let op = later(|| {
            thread::sleep(Duration::from_millis(20));
            0
        });
        let result = test_await(op);
        assert_eq!(result, 0);
    }

    #[test]
    fn stress() {
        let start = Instant::now();
        let count = 1000u32;
        let mut ops: Vec<_> = (0..count).map(move |v| later(move || v)).collect();
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
