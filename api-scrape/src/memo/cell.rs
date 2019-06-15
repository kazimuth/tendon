// TODO: rename to MemoCell?
// TODO: check all atomic returns

use std::{
    cell::{Cell, UnsafeCell},
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    task,
};

const INCOMPLETE: usize = 0;
const STARTED: usize = 1;
const STARTED_WAKELOCKED: usize = 2;
const COMPLETE: usize = 3;

pub struct Memo<T: Future + Send> {
    inner: Arc<MemoInner<T>>,
    driver: Cell<bool>,
}
impl<T: Future + Send> Memo<T> {
    pub fn new(future: T) -> Memo<T> {
        Memo {
            inner: Arc::new(MemoInner {
                state: AtomicUsize::new(INCOMPLETE),
                future: UnsafeCell::new(Some(Box::pin(future))),
                result: UnsafeCell::new(None),
                wakers: UnsafeCell::new(Vec::new()),
            }),
            driver: Cell::new(false),
        }
    }

    fn transition(&self, from: usize, to: usize) -> usize {
        self.inner
            .state
            .compare_and_swap(from, to, Ordering::SeqCst)
    }

    fn drive(&'a self, cx: &mut task::Context) -> task::Poll<&T::Output> {
        assert!(self.driver.get(), "can't drive from non-driver");

        let result = unsafe {
            (*self.inner.future.get())
                .as_mut()
                .expect("memo: invariant violated")
                .as_mut()
                .poll(cx)
        };

        let result = match result {
            task::Poll::Pending => return task::Poll::Pending,
            task::Poll::Ready(result) => result,
        };

        unsafe {
            *self.inner.result.get() = Some(result);
            *self.inner.future.get() = None;
            while self.transition(STARTED, COMPLETE) != STARTED {}

            let wakers = &mut *self.inner.wakers.get();
            for waker in wakers.drain(..) {
                waker.wake();
            }
            wakers.shrink_to_fit();

            self.get_result()
        }
    }

    /// Get the result of the future.
    /// Only safe to call when self.inner.state == COMPLETE
    unsafe fn get_result<'a>(&'a self) -> task::Poll<&'a T::Output> {
        task::Poll::Ready(
            (*self.inner.result.get())
                .as_ref()
                .expect("memo: invariants violated"),
        )
    }
}

impl<'a, T: Future + Send> Future for &'a Memo<T> {
    type Output = &'a T::Output;

    #[inline(never)]
    fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context) -> task::Poll<&'a T::Output> {
        if self.driver.get() {
            return self.drive(cx);
        }
        match self.transition(INCOMPLETE, STARTED) {
            INCOMPLETE => {
                // we've just become the driver for this future.
                self.driver.set(true);
                self.drive(cx)
            }
            STARTED | STARTED_WAKELOCKED => {
                // we're awaiting the future.

                // lock wakers:
                // TODO: this deadlocks on completion, fix!!!
                while self.transition(STARTED, STARTED_WAKELOCKED) != STARTED {}

                unsafe { &mut *self.inner.wakers.get() }.push(cx.waker().clone());

                // unlock wakers:
                self.inner.state.store(STARTED, Ordering::SeqCst);

                task::Poll::Pending
            }
            COMPLETE => {
                // future is already complete.
                unsafe { self.get_result() }
            }
            other => panic!("memo: impossible state: {}", other),
        }
    }
}
impl<T: Future + Send> Clone for Memo<T> {
    fn clone(&self) -> Self {
        Memo {
            inner: self.inner.clone(),
            driver: Cell::new(false),
        }
    }
}

struct MemoInner<T: Future + Send> {
    state: AtomicUsize,
    future: UnsafeCell<Option<Pin<Box<T>>>>,
    result: UnsafeCell<Option<T::Output>>,
    wakers: UnsafeCell<Vec<task::Waker>>,
}
unsafe impl<T: Future + Send> Send for MemoInner<T> {}
unsafe impl<T: Future + Send> Sync for MemoInner<T> {}

#[cfg(test)]
mod tests {
    use super::Memo;

    #[runtime::test]
    async fn memo_basic() {
        let m = Memo::new(async { 3u8 });
        let m2 = m.clone();
        let m2 = (&m2).await;
        let m = (&m).await;
        assert_eq!(m2, m);
        assert_eq!(*m, 3);
    }
}
