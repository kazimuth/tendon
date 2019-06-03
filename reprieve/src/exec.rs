use lazy_static::lazy_static;
use log::info;
use parking_lot::Mutex;
use std::cell::Cell;
use std::collections::VecDeque;
use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::ptr;
use std::sync::mpsc::sync_channel;
use std::sync::Arc;
use std::task::{Context, Poll, RawWaker, RawWakerVTable};
use std::thread;
use std::thread_local;
use std::time::Instant;

/// Start another future
pub fn spawn<T: Send + 'static, F: Future<Output = T> + Send + 'static>(
    input: F,
) -> impl Future<Output = T> {
    let (sender, future) = crate::once_future::<T>();

    spawn_raw(async move {
        let result = input.await;
        sender.set(result);
    });

    future
}

pub fn wait<T: Send + 'static, F: Future<Output = T> + Send + 'static>(input: F) -> T {
    let (sx, rx) = sync_channel(1);
    let mut backoff = crate::backoff::Backoff::new(50);

    spawn_raw(async move {
        let result = input.await;
        sx.send(result).expect("poison");
    });

    loop {
        if let Ok(result) = rx.try_recv() {
            return result;
        }
        backoff.wait();
    }
}

fn spawn_raw<F: Future<Output = ()> + Send + 'static>(f: F) {
    let handle = TaskHandle(Arc::new(Mutex::new(Task {
        state: State::InQueue,
        future: Box::pin(f),
    })));
    RUNTIME.push_random(handle);
}

lazy_static! {
    static ref RUNTIME: Runtime = init_runtime();
}
struct Runtime {
    queues: Vec<Mutex<VecDeque<TaskHandle>>>,
}
impl Runtime {
    fn push_random(&self, handle: TaskHandle) {
        self.queues[rand() % self.queues.len()]
            .lock()
            .push_back(handle);
    }
}

fn init_runtime() -> Runtime {
    let mut queues = vec![];

    info!("starting reprieve executor thread pool");

    for i in 0..num_cpus::get() {
        queues.push(Mutex::new(VecDeque::new()));
        let name = format!(
            "reprieve {} executor worker {}",
            env!("CARGO_PKG_VERSION"),
            i
        );
        info!("starting thread `{}`", &name);
        thread::Builder::new()
            .name(name)
            .spawn(move || {
                let mut backoff = crate::backoff::Backoff::new(250);
                loop {
                    let mut handle = RUNTIME.queues[i].lock().pop_front();
                    let mut tries = 0;
                    while handle.is_none() && tries < 3 {
                        tries += 1;
                        handle = RUNTIME.queues[rand() % RUNTIME.queues.len()]
                            .lock()
                            .pop_front();
                    }
                    if let Some(handle) = handle {
                        backoff.reset();
                        let mut task = handle.0.lock();
                        assert!(task.state == State::InQueue);
                        let waker = handle.clone().into();
                        let mut context = Context::from_waker(&waker);
                        match task.future.as_mut().poll(&mut context) {
                            Poll::Ready(()) => {
                                task.state = State::Finished;
                            }
                            Poll::Pending => {
                                task.state = State::Pending;
                            }
                        }
                    } else {
                        backoff.wait();
                    }
                }
            })
            .expect("failed to start runtime");
    }

    Runtime { queues }
}

struct Task {
    state: State,
    future: Pin<Box<dyn Future<Output = ()> + Send>>,
}

#[derive(PartialEq, Eq)]
enum State {
    Pending,
    InQueue,
    Finished,
}

#[derive(Clone)]
struct TaskHandle(Arc<Mutex<Task>>);

impl TaskHandle {
    fn wake(&self) {
        // try to lock: the only other thing that locks a task is another waker or the executor.
        // in either case, we don't need to do anything.
        let task = self.0.try_lock();
        if let Some(mut task) = task {
            if task.state == State::Pending {
                task.state = State::InQueue;
                RUNTIME.push_random(TaskHandle(self.0.clone()))
            }
        }
    }
}

// VTable nonsense
impl Into<std::task::Waker> for TaskHandle {
    fn into(self) -> std::task::Waker {
        unsafe { std::task::Waker::from_raw(RawWaker::new(data(self), &*VTABLE)) }
    }
}
fn data(handle: TaskHandle) -> *const () {
    unsafe { mem::transmute::<TaskHandle, *const ()>(handle) }
}
fn load(p: *const ()) -> TaskHandle {
    if p == ptr::null_mut() {
        panic!("null waker!")
    }
    unsafe { mem::transmute::<*const (), TaskHandle>(p) }
}
fn load_ref<'a>(p: &'a *const ()) -> &'a TaskHandle {
    if *p == ptr::null_mut() {
        panic!("null waker!")
    }
    unsafe { mem::transmute::<&*const (), &TaskHandle>(p) }
}
lazy_static! {
    static ref VTABLE: RawWakerVTable = RawWakerVTable::new(
        // clone
        |p| { RawWaker::new(data(load_ref(&p).clone()), &*VTABLE) },
        // wake
        |p| { load(p).wake(); },
        // wake by ref
        |p| { load_ref(&p).wake(); },
        // drop
        |p| { load(p); }
    );
}

// simple rng
lazy_static! {
    static ref RAND_START: Instant = Instant::now();
}
thread_local! {
    static RAND: Cell<u32> = Cell::new(RAND_START.elapsed().subsec_nanos());
}
fn rand() -> usize {
    RAND.with(|rand| {
        // xorshift; no need to bring in all of rand, lol
        let mut x = rand.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        rand.set(x);
        x as usize
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let t = async { 3 };
        let result = wait(t);
        assert_eq!(result, 3);
    }

    #[test]
    fn stress() {
        let start = Instant::now();
        let count = 10000;

        let ops: Vec<_> = (0..count).map(|v| spawn(async move { v })).collect();

        wait(async {
            for op in ops {
                op.await;
            }
        });

        println!(
            "exec stress elapsed: {:?} ({:?} per item)",
            start.elapsed(),
            start.elapsed() / count
        );
    }
}
