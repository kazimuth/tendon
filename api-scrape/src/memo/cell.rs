use parking_lot::RwLock;
use std::{
    cell::UnsafeCell,
    future::Future,
    ops::Deref,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering::SeqCst},
        Arc,
    },
    task,
};

const UNSTARTED: usize = 0;
const STARTED: usize = 1;
const FINISHED: usize = 2;

/// A memoized future.
/// Can be cloned and .awaited() as many times as you'd like; only the first .await will run.
/// Only shared references can be taken to the result. Use RwLock or Mutex if you need mutability.
/// ```
/// #![feature(async_await)]
/// # fn expensive_computation() -> usize { 3 }
/// use api_scrape::memo::Memo;
///
/// #[runtime::main]
/// async fn main() {
///     let result = Memo::new(async {
///         expensive_computation();
///     });
///     let other = result.clone();
///     other.await; // runs the computation
///     result.await; // reuses result
/// }
/// ```
pub struct Memo<R> {
    inner: Option<Arc<MemoInner<dyn Future<Output = R> + Send + Sync>>>,
    result: MemoResult<R>,
    driver: bool,
}
/// The result of a memoized future.
pub struct MemoResult<R>(Arc<UnsafeCell<Option<R>>>);
struct MemoInner<T: ?Sized> {
    state: AtomicUsize,
    future: RwLock<Pin<Box<T>>>,
    wakers: RwLock<Vec<task::Waker>>,
}
unsafe impl<R: Sync> Send for MemoResult<R> {}
unsafe impl<R: Sync> Sync for MemoResult<R> {}
impl<R> Clone for MemoResult<R>
where
    R: Send + Sync,
{
    fn clone(&self) -> Self {
        MemoResult(self.0.clone())
    }
}

impl<R> Memo<R>
where
    R: Send + Sync,
{
    pub fn new<F>(future: F) -> Memo<R>
    where
        F: Future<Output = R> + Send + Sync + 'static,
    {
        Memo {
            inner: Some(Arc::new(MemoInner {
                state: AtomicUsize::new(UNSTARTED),
                future: RwLock::new(Box::pin(future)),
                wakers: RwLock::new(Vec::new()),
            })),
            result: MemoResult(Arc::new(UnsafeCell::new(None))),
            driver: false,
        }
    }

    fn drive(&mut self, cx: &mut task::Context) {
        assert!(self.driver, "memo: can't drive from non-driver");

        {
            let inner = self.inner.as_ref().expect("memo: nothing to drive");
            let result = inner
                .future
                .try_write()
                .expect("memo: multiple drivers? or recursion")
                .as_mut()
                .poll(cx);

            let result = match result {
                task::Poll::Pending => return, // nothing to do; future has scheduled wakening
                task::Poll::Ready(result) => result,
            };
            unsafe {
                *self.result.0.get() = Some(result);
            }
            inner.state.store(FINISHED, SeqCst);
            {
                let mut wakers = inner.wakers.write();

                for waker in wakers.drain(..) {
                    waker.wake();
                }

                wakers.shrink_to_fit();
            }
        };
    }
}

impl<R> Future for Memo<R>
where
    R: Send + Sync,
{
    type Output = MemoResult<R>;

    #[inline(never)]
    fn poll(self: Pin<&mut Self>, cx: &mut task::Context) -> task::Poll<MemoResult<R>> {
        let self_ = unsafe { self.get_unchecked_mut() };
        if self_.inner.is_none() {
            return task::Poll::Ready(self_.result.clone());
        }
        {
            let inner = self_.inner.as_ref().expect("memo: invariant violated");
            let pre = inner.state.compare_and_swap(UNSTARTED, STARTED, SeqCst);
            if pre == UNSTARTED {
                // we're driver now
                self_.driver = true;
            } else if pre == FINISHED {
                self_.inner = None;
                return task::Poll::Ready(self_.result.clone());
            }
        }
        if self_.driver {
            self_.drive(cx);
        }
        {
            if self_
                .inner
                .as_ref()
                .expect("memo: invariant violated")
                .state
                .load(SeqCst)
                == FINISHED
            {
                self_.inner = None;
                // we were just blocked on the driver trying to wake us up
                // skip it
                task::Poll::Ready(self_.result.clone())
            } else {
                self_
                    .inner
                    .as_ref()
                    .expect("memo: invariant violated")
                    .wakers
                    .write()
                    .push(cx.waker().clone());
                task::Poll::Pending
            }
        }
    }
}
impl<R> Clone for Memo<R>
where
    R: Send + Sync,
{
    fn clone(&self) -> Self {
        Memo {
            inner: self.inner.clone(),
            result: self.result.clone(),
            driver: false,
        }
    }
}

impl<R: Sync> Deref for MemoResult<R> {
    type Target = R;
    fn deref(&self) -> &Self::Target {
        (unsafe { &*self.0.get() })
            .as_ref()
            .expect("memo: result returned before finish")
    }
}

#[cfg(test)]
mod tests {
    use super::Memo;

    #[runtime::test]
    async fn memo_basic() {
        let m = Memo::new(async { 3u8 });
        let m2 = m.clone();
        let m2 = m2.await;
        let m = m.await;
        assert_eq!(*m2, *m);
        assert_eq!(*m, 3);
    }
}
