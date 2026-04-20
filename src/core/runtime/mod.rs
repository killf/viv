pub mod channel;
pub mod executor;
pub mod join;
pub mod reactor;
pub mod task;
pub mod timer;

pub use channel::{AsyncReceiver, NotifySender, async_channel};
pub use executor::{Executor, block_on, block_on_local, noop_waker};
pub use join::{join, join_all};
pub use reactor::with_reactor;
pub use task::JoinHandle;
pub use timer::sleep;

use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc;
use std::thread;

type SpawnFn = Box<dyn FnOnce(&mut Executor) + Send>;

pub struct Runtime {
    _handle: thread::JoinHandle<()>,
    tx: mpsc::Sender<SpawnFn>,
}

impl Runtime {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<SpawnFn>();
        let handle = thread::spawn(move || {
            let mut exec = Executor::new();
            loop {
                // 接收新任务提交
                loop {
                    match rx.try_recv() {
                        Ok(f) => f(&mut exec),
                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
                    }
                }
                let did_work = exec.run_ready();
                if !did_work {
                    with_reactor(|r| r.wait(std::time::Duration::from_millis(10))).ok();
                }
            }
        });
        Runtime {
            _handle: handle,
            tx,
        }
    }

    pub fn spawn(&self, f: impl FnOnce(&mut Executor) + Send + 'static) {
        self.tx.send(Box::new(f)).ok();
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrapper that asserts a Future is Send.
///
/// SAFETY: Only safe when the future runs inside `block_on_local` (single-threaded).
/// The wrapped future is never actually sent between threads.
pub struct AssertSend<F>(pub F);

unsafe impl<F: Future> Send for AssertSend<F> {}

impl<F: Future> Future for AssertSend<F> {
    type Output = F::Output;
    fn poll(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        unsafe { self.map_unchecked_mut(|s| &mut s.0).poll(cx) }
    }
}
