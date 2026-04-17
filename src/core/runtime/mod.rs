pub mod task;
pub mod executor;
pub mod reactor;
pub mod timer;
pub mod channel;

pub use executor::{block_on, Executor};
pub use reactor::reactor;
pub use timer::sleep;
pub use task::JoinHandle;
pub use channel::{async_channel, NotifySender, AsyncReceiver};

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
                loop {
                    match rx.try_recv() {
                        Ok(f) => f(&mut exec),
                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
                    }
                }
                let did_work = exec.run_ready();
                if !did_work {
                    reactor().lock().unwrap().wait(std::time::Duration::from_millis(10));
                }
            }
        });
        Runtime { _handle: handle, tx }
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
