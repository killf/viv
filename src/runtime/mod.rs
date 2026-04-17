pub mod task;
pub mod executor;
pub mod reactor;
pub mod timer;

pub use executor::{block_on, Executor};
pub use reactor::reactor;
pub use timer::sleep;
pub use task::JoinHandle;

use std::sync::mpsc;
use std::thread;

/// Runtime 运行在独立线程，暴露 spawn 接口
pub struct Runtime {
    _handle: thread::JoinHandle<()>,
    tx: mpsc::Sender<Box<dyn FnOnce(&mut Executor) + Send>>,
}

impl Runtime {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<Box<dyn FnOnce(&mut Executor) + Send>>();
        let handle = thread::spawn(move || {
            let mut exec = Executor::new();
            loop {
                // 接收新任务提交
                while let Ok(f) = rx.try_recv() {
                    f(&mut exec);
                }
                exec.run_ready();
                if exec.is_idle() {
                    reactor().lock().unwrap().wait(std::time::Duration::from_millis(10));
                }
            }
        });
        Runtime { _handle: handle, tx }
    }

    pub fn spawn<T>(&self, f: impl FnOnce(&mut Executor) + Send + 'static) {
        self.tx.send(Box::new(f)).ok();
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
