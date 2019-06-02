use once_cell::sync::OnceCell;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

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

pub fn once_future<T: Send + 'static>() -> (Sender<T>, OnceFuture<T>) {
    let inner = Arc::new(FutureInner {
        waker: OnceCell::new(),
        result: OnceCell::new(),
    });
    (
        Sender {
            inner: inner.clone(),
            phantom: PhantomData,
        },
        OnceFuture {
            inner,
            phantom: PhantomData,
            returned: false,
        },
    )
}

pub struct OnceFuture<T> {
    // required to prevent accidental aliasing if poll() is called after Ready() is returned
    returned: bool,
    inner: Arc<FutureInner>,
    phantom: PhantomData<T>,
}

impl<T> Future for OnceFuture<T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        if self.returned {
            panic!("OnceFuture polled after returning")
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

pub struct Sender<T> {
    inner: Arc<FutureInner>,
    phantom: PhantomData<T>,
}
impl<T: Send + 'static> Sender<T> {
    pub fn set(&self, t: T) -> bool {
        let raw = Box::into_raw(Box::new(t)) as *mut ();

        // EVENT P1
        if !self.inner.result.set(SendPtr(raw)).is_ok() {
            return false;
        }

        // EVENT P2
        if let Some(waker) = self.inner.waker.get() {
            waker.wake_by_ref();
        }

        true
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
