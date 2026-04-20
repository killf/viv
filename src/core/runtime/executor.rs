use super::task::{JoinHandle, Task, TaskId, oneshot};
use crate::core::sync::lock_or_recover;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, mpsc};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::time::Duration;

/// Create a no-op Waker that does nothing on wake/clone/drop.
pub fn noop_waker() -> Waker {
    const VTABLE: RawWakerVTable =
        RawWakerVTable::new(|p| RawWaker::new(p, &VTABLE), |_| {}, |_| {}, |_| {});
    unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
}

fn wait_for_io() {
    crate::core::runtime::reactor::with_reactor(|r| r.wait(Duration::from_millis(10))).ok();
}

pub struct Executor {
    tasks: HashMap<TaskId, Arc<Task>>,
    ready_tx: mpsc::Sender<TaskId>,
    ready_rx: mpsc::Receiver<TaskId>,
    next_id: TaskId,
}

impl Executor {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Executor {
            tasks: HashMap::new(),
            ready_tx: tx,
            ready_rx: rx,
            next_id: 0,
        }
    }

    pub fn spawn<T>(&mut self, future: impl Future<Output = T> + Send + 'static) -> JoinHandle<T>
    where
        T: Send + 'static + Unpin,
    {
        let id = self.next_id;
        self.next_id += 1;
        let (tx, rx) = oneshot::<T>();
        let wrapped = async move {
            let result = future.await;
            tx.send(result);
        };
        let task = Task::new(id, wrapped, self.ready_tx.clone());
        self.tasks.insert(id, task);
        self.ready_tx.send(id).ok();
        JoinHandle(rx)
    }

    pub fn run_ready(&mut self) -> bool {
        let mut did_work = false;
        while let Ok(id) = self.ready_rx.try_recv() {
            self.poll_task(id);
            did_work = true;
        }
        did_work
    }

    fn poll_task(&mut self, id: TaskId) {
        let task = match self.tasks.get(&id) {
            Some(t) => t.clone(),
            None => return,
        };
        let waker = task.waker();
        let mut cx = Context::from_waker(&waker);
        let mut future = lock_or_recover(&task.future);
        if let Poll::Ready(()) = future.as_mut().poll(&mut cx) {
            drop(future);
            self.tasks.remove(&id);
        }
    }

    pub fn is_idle(&self) -> bool {
        self.tasks.is_empty()
    }

    pub fn sender(&self) -> mpsc::Sender<TaskId> {
        self.ready_tx.clone()
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

/// Block current thread on a future (no Send required).
pub fn block_on_local<T>(mut future: impl Future<Output = T> + Unpin) -> T {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    let mut exec = Executor::new();
    loop {
        exec.run_ready();
        if let Poll::Ready(v) = Pin::new(&mut future).poll(&mut cx) {
            return v;
        }
        wait_for_io();
    }
}

/// Block current thread on a future (requires Send + 'static).
pub fn block_on<T: Unpin + Send + 'static>(future: impl Future<Output = T> + Send + 'static) -> T {
    let mut exec = Executor::new();
    let mut handle = exec.spawn(future);
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    loop {
        let did_work = exec.run_ready();
        if let Poll::Ready(v) = Pin::new(&mut handle).poll(&mut cx) {
            return v;
        }
        if !did_work {
            wait_for_io();
        }
    }
}
