// TODO: rename to MemoCell?
// TODO: check all atomic returns
// TODO: handle drive() panics
// TODO: handle recursion?

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
const PANIC: usize = 4;


pub struct Memo<T: Future + Send + Sync> {
    inner: Arc<MemoInner<T>>,
    driver: Cell<bool>,
}
impl<T: Future + Send + Sync> Memo<T> {
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

    fn drive(&self, cx: &mut task::Context) -> task::Poll<&T::Output> {
        assert!(self.driver.get(), "can't drive from non-driver");

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
            (*self.inner.future.get())
                .as_mut()
                .expect("memo: invariant violated")
                .as_mut()
                .poll(cx)
        }));
        let result = match result {
            Ok(result) => result,
            Err(cause) => {
                loop {
                    let pre = self.transition(STARTED, PANIC);
                    if pre == STARTED_WAKELOCKED {
                        continue;
                    } else if pre == STARTED_WAKELOCKED {
                        break;
                    } else {
                        eprintln!("memo: weird state during panic: {}", pre);
                        break;
                    }
                }
                std::panic::resume_unwind(cause);
            }
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

impl<'a, T: Future + Send + Sync> Future for &'a Memo<T> {
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

                let waker = cx.waker().clone();

                loop {
                    let pre = self.transition(STARTED, STARTED_WAKELOCKED);
                    if pre == STARTED {
                        // add our waker to the list
                        unsafe { &mut *self.inner.wakers.get() }.push(waker);
                        self.inner.state.store(STARTED, Ordering::SeqCst);
                        break;
                    } else if pre == STARTED_WAKELOCKED {
                        // yeah yeah wait around. this'll only happen for a few nanoseconds.
                        continue;
                    } else {
                        // something else happened. try again.
                        return self.poll(cx);
                    }
                }
                task::Poll::Pending
            }
            COMPLETE => {
                // future is already complete.
                unsafe { self.get_result() }
            }
            PANIC => {
                panic!("memo: panicked")
            }
            other => panic!("memo: impossible state: {}", other),
        }
    }
}
impl<T: Future + Send + Sync> Clone for Memo<T> {
    fn clone(&self) -> Self {
        Memo {
            inner: self.inner.clone(),
            driver: Cell::new(false),
        }
    }
}

struct MemoInner<T: Future + Send + Sync> {
    state: AtomicUsize,
    future: UnsafeCell<Option<Pin<Box<T>>>>,
    result: UnsafeCell<Option<T::Output>>,
    wakers: UnsafeCell<Vec<task::Waker>>,
}
unsafe impl<T: Future + Send + Sync> Send for MemoInner<T> {}
unsafe impl<T: Future + Send + Sync> Sync for MemoInner<T> {}

pub struct MemoResult<T: Future + Send + Sync> {
    inner: Arc<MemoInner<T>
}
impl<T: Future + Send + Sync> Deref for MemoResult<T> {
    type Target = T::Output;
    fn deref(&self) -> T::Output {
        unsafe {
            
        }
    }
}

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
