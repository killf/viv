use crate::Error;
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
    // `None` means the platform reactor failed to initialize. All operations
    // return an error in that case so callers can propagate and shut down
    // gracefully instead of crashing.
    inner: Option<PlatformReactor>,
    init_error: Option<String>,
}

fn unavailable(op: &str, reason: Option<&str>) -> Error {
    let reason = reason.unwrap_or("unknown");
    Error::Io(std::io::Error::other(format!(
        "reactor unavailable for {op}: {reason}"
    )))
}

impl Reactor {
    fn new() -> Self {
        match PlatformReactor::new() {
            Ok(inner) => Reactor {
                inner: Some(inner),
                init_error: None,
            },
            Err(e) => Reactor {
                inner: None,
                init_error: Some(format!("{e}")),
            },
        }
    }

    pub fn register_readable(
        &mut self,
        handle: RawHandle,
        waker: Waker,
    ) -> crate::Result<u64> {
        let reason = self.init_error.clone();
        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| unavailable("register_readable", reason.as_deref()))?;
        inner.register_read(handle, waker)
    }

    pub fn register_writable(
        &mut self,
        handle: RawHandle,
        waker: Waker,
    ) -> crate::Result<u64> {
        let reason = self.init_error.clone();
        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| unavailable("register_writable", reason.as_deref()))?;
        inner.register_write(handle, waker)
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
            // No reactor: avoid a tight busy loop by sleeping the timeout.
            std::thread::sleep(timeout);
        }
    }

    /// Access the underlying platform reactor if available.
    pub fn platform(&self) -> Option<&PlatformReactor> {
        self.inner.as_ref()
    }
}
