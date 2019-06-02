/*
        let raw = Box::into_raw(Box::new(result)) as *mut ();

        // EVENT P1
        if !inner.result.set(SendPtr(raw)).is_ok() {
            panic!("invariants violated");
        }

        // EVENT P2
        if let Some(waker) = inner.waker.get() {
            waker.wake_by_ref();
        }

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

*/
