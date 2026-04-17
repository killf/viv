use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, mpsc};
use std::task::{Context, Poll};
use std::time::Duration;
use super::task::{Task, TaskId, JoinHandle, oneshot};

pub struct Executor {
    tasks: HashMap<TaskId, Arc<Task>>,
    ready_tx: mpsc::Sender<TaskId>,
    ready_rx: mpsc::Receiver<TaskId>,
    next_id: TaskId,
}

impl Executor {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Executor { tasks: HashMap::new(), ready_tx: tx, ready_rx: rx, next_id: 0 }
    }

    pub fn spawn<T>(&mut self, future: impl Future<Output = T> + Send + 'static) -> JoinHandle<T>
    where
        T: Send + 'static + Unpin,
    {
        let id = self.next_id;
        self.next_id += 1;
        let (tx, rx) = oneshot::<T>();
        // 包装 future：完成时把结果发给 JoinHandle
        let wrapped = async move {
            let result = future.await;
            tx.send(result);
        };
        let task = Task::new(id, wrapped, self.ready_tx.clone());
        self.tasks.insert(id, task);
        self.ready_tx.send(id).ok();
        JoinHandle(rx)
    }

    /// 排干就绪队列，poll 所有就绪任务一次。返回是否处理了任何任务。
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
        let mut future = task.future.lock().unwrap();
        if let Poll::Ready(()) = future.as_mut().poll(&mut cx) {
            drop(future);
            self.tasks.remove(&id);
        }
    }

    pub fn is_idle(&self) -> bool { self.tasks.is_empty() }

    pub fn sender(&self) -> mpsc::Sender<TaskId> { self.ready_tx.clone() }
}

impl Default for Executor {
    fn default() -> Self { Self::new() }
}

/// 阻塞当前线程运行 future 至完成（不要求 Send）
pub fn block_on_local<T>(mut future: impl Future<Output = T> + Unpin) -> T {
    use std::task::{RawWaker, RawWakerVTable, Waker};
    const NOOP_VTABLE: RawWakerVTable = RawWakerVTable::new(
        |p| RawWaker::new(p, &NOOP_VTABLE),
        |_| {}, |_| {}, |_| {},
    );
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &NOOP_VTABLE)) };
    let mut cx = Context::from_waker(&waker);

    let mut exec = Executor::new();
    loop {
        exec.run_ready();
        if let Poll::Ready(v) = Pin::new(&mut future).poll(&mut cx) {
            return v;
        }
        if let Ok(mut r) = crate::core::runtime::reactor::reactor().try_lock() {
            r.wait(Duration::from_millis(10));
        } else {
            std::thread::yield_now();
        }
    }
}

/// 阻塞当前线程运行 future 至完成（要求 Send + 'static）
pub fn block_on<T: Unpin + Send + 'static>(
    future: impl Future<Output = T> + Send + 'static,
) -> T {
    let mut exec = Executor::new();
    let mut handle = exec.spawn(future);

    // noop waker 用于 poll JoinHandle
    use std::task::{RawWaker, RawWakerVTable, Waker};
    const NOOP_VTABLE: RawWakerVTable = RawWakerVTable::new(
        |p| RawWaker::new(p, &NOOP_VTABLE),
        |_| {}, |_| {}, |_| {},
    );
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &NOOP_VTABLE)) };
    let mut cx = Context::from_waker(&waker);

    loop {
        let did_work = exec.run_ready();
        if let Poll::Ready(v) = Pin::new(&mut handle).poll(&mut cx) {
            return v;
        }
        // 就绪队列为空（无论是否还有挂起的任务），等待 reactor I/O 事件
        if !did_work {
            if let Ok(mut r) = crate::core::runtime::reactor::reactor().try_lock() {
                r.wait(Duration::from_millis(10));
            } else {
                std::thread::yield_now();
            }
        }
    }
}
