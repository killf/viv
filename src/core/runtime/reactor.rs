use crate::Error;
use crate::core::platform::{PlatformAsyncReactor, RawHandle};
use std::cell::RefCell;
use std::task::Waker;
use std::time::Duration;

thread_local! {
    static REACTOR: RefCell<Reactor> = RefCell::new(Reactor::new());
}

/// Access the current thread's reactor. Returns an error if called re-entrantly.
pub fn with_reactor<F, R>(f: F) -> crate::Result<R>
where
    F: FnOnce(&mut Reactor) -> R,
{
    REACTOR.with(|cell| {
        cell.try_borrow_mut()
            .map(|mut r| f(&mut r))
            .map_err(|_| Error::Invariant("reactor re-entrantly borrowed".into()))
    })
}

pub struct Reactor {
    inner: Option<PlatformAsyncReactor>,
    init_error: Option<String>,
}

fn unavailable(op: &str, reason: Option<&str>) -> Error {
    Error::Io(std::io::Error::other(format!(
        "reactor unavailable for {op}: {}",
        reason.unwrap_or("unknown")
    )))
}

impl Reactor {
    fn new() -> Self {
        match PlatformAsyncReactor::new() {
            Ok(inner) => Reactor { inner: Some(inner), init_error: None },
            Err(e) => Reactor { inner: None, init_error: Some(format!("{e}")) },
        }
    }

    pub fn register_readable(&mut self, handle: RawHandle, waker: Waker) -> crate::Result<u64> {
        let reason = self.init_error.clone();
        self.inner
            .as_mut()
            .ok_or_else(|| unavailable("register_readable", reason.as_deref()))?
            .register_read(handle, waker)
    }

    pub fn register_writable(&mut self, handle: RawHandle, waker: Waker) -> crate::Result<u64> {
        let reason = self.init_error.clone();
        self.inner
            .as_mut()
            .ok_or_else(|| unavailable("register_writable", reason.as_deref()))?
            .register_write(handle, waker)
    }

    pub fn remove(&mut self, token: u64) {
        if let Some(inner) = self.inner.as_mut() {
            inner.deregister(token).ok();
        }
    }

    pub fn wait(&mut self, timeout: Duration) {
        if let Some(inner) = self.inner.as_mut() {
            inner.poll(timeout).ok();
        } else {
            std::thread::sleep(timeout);
        }
    }

    pub fn platform(&self) -> Option<&PlatformAsyncReactor> {
        self.inner.as_ref()
    }
}
