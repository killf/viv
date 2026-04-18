use crate::core::platform::{PlatformReactor, RawHandle};
use std::sync::{Arc, Mutex, OnceLock};
use std::task::Waker;
use std::time::Duration;

static REACTOR: OnceLock<Arc<Mutex<Reactor>>> = OnceLock::new();

pub fn reactor() -> Arc<Mutex<Reactor>> {
    REACTOR
        .get_or_init(|| Arc::new(Mutex::new(Reactor::new())))
        .clone()
}

pub struct Reactor {
    inner: PlatformReactor,
}

impl Reactor {
    fn new() -> Self {
        Reactor {
            inner: PlatformReactor::new().expect("reactor init"),
        }
    }

    pub fn register_readable(&mut self, handle: RawHandle, waker: Waker) -> u64 {
        match self.inner.register_read(handle, waker) {
            Ok(tok) => tok,
            Err(e) => panic!("register_read failed: handle={:?} err={:?}", handle, e),
        }
    }

    pub fn register_writable(&mut self, handle: RawHandle, waker: Waker) -> u64 {
        match self.inner.register_write(handle, waker) {
            Ok(tok) => tok,
            Err(e) => panic!("register_write failed: handle={:?} err={:?}", handle, e),
        }
    }

    pub fn remove(&mut self, token: u64) {
        self.inner.deregister(token).ok();
    }

    pub fn wait(&mut self, timeout: Duration) {
        self.inner.poll(timeout).ok();
    }

    /// Access the underlying platform reactor.
    pub fn platform(&self) -> &PlatformReactor {
        &self.inner
    }
}
