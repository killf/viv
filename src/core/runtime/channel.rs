use std::cell::UnsafeCell;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::task::{Context, Poll};

use super::reactor::reactor;
use crate::core::platform::PlatformNotifier;

struct NotifierHandle {
    inner: PlatformNotifier,
    ref_count: AtomicUsize,
}

// ── NotifySender ─────────────────────────────────────────────────────────────

pub struct NotifySender<T> {
    tx: mpsc::Sender<T>,
    notifier: Arc<NotifierHandle>,
}

impl<T> Clone for NotifySender<T> {
    fn clone(&self) -> Self {
        self.notifier.ref_count.fetch_add(1, Ordering::Relaxed);
        NotifySender {
            tx: self.tx.clone(),
            notifier: Arc::clone(&self.notifier),
        }
    }
}

impl<T> Drop for NotifySender<T> {
    fn drop(&mut self) {
        if self.notifier.ref_count.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.notifier.inner.notify().ok();
        }
    }
}

impl<T> NotifySender<T> {
    pub fn send(&self, value: T) -> crate::Result<()> {
        self.tx.send(value).map_err(|_| {
            crate::Error::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "receiver dropped",
            ))
        })?;
        self.notifier.inner.notify().ok();
        Ok(())
    }
}

// ── AsyncReceiver ────────────────────────────────────────────────────────────

pub struct AsyncReceiver<T> {
    rx: mpsc::Receiver<T>,
    notifier: Arc<NotifierHandle>,
    token: UnsafeCell<Option<u64>>,
}

unsafe impl<T: Send> Send for AsyncReceiver<T> {}
unsafe impl<T: Send> Sync for AsyncReceiver<T> {}

impl<T> Drop for AsyncReceiver<T> {
    fn drop(&mut self) {
        let token = self.token.get_mut();
        if let Some(t) = token.take() {
            reactor().lock().unwrap().remove(t);
        }
    }
}

impl<T> AsyncReceiver<T> {
    pub fn recv(&self) -> RecvFuture<'_, T> {
        RecvFuture { receiver: self }
    }
}

// ── RecvFuture ───────────────────────────────────────────────────────────────

pub struct RecvFuture<'a, T> {
    receiver: &'a AsyncReceiver<T>,
}

impl<T> Future for RecvFuture<'_, T> {
    type Output = crate::Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.receiver.notifier.inner.drain().ok();

        match self.receiver.rx.try_recv() {
            Ok(value) => Poll::Ready(Ok(value)),
            Err(mpsc::TryRecvError::Disconnected) => Poll::Ready(Err(crate::Error::Io(
                std::io::Error::new(std::io::ErrorKind::BrokenPipe, "sender dropped"),
            ))),
            Err(mpsc::TryRecvError::Empty) => {
                let handle = self.receiver.notifier.inner.handle();
                let r = reactor();
                let mut guard = r.lock().unwrap();
                // Clean up any previous registration so we can re-register with
                // the new waker. epoll_ctl(ADD) is not idempotent — the second
                // call on an already-registered fd returns EEXIST, which would
                // panic in register_readable. This matches the pattern used in
                // async_tcp.rs for spurious re-polls.
                if let Some(old) = unsafe { (*self.receiver.token.get()).take() } {
                    guard.remove(old);
                }
                let token = guard.register_readable(handle, cx.waker().clone());
                unsafe { *self.receiver.token.get() = Some(token) };
                Poll::Pending
            }
        }
    }
}

// ── Constructor ──────────────────────────────────────────────────────────────

pub fn async_channel<T>() -> (NotifySender<T>, AsyncReceiver<T>) {
    let (tx, rx) = mpsc::channel();
    let notifier = Arc::new(NotifierHandle {
        inner: PlatformNotifier::new().expect("create notifier"),
        ref_count: AtomicUsize::new(1),
    });
    let sender = NotifySender {
        tx,
        notifier: Arc::clone(&notifier),
    };
    let receiver = AsyncReceiver {
        rx,
        notifier,
        token: UnsafeCell::new(None),
    };
    (sender, receiver)
}
