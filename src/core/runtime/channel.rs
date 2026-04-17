use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::SendError;
use std::task::{Context, Poll, Waker};

// ─────────────────────────────────────────────────────────────────────────────
// Shared state
// ─────────────────────────────────────────────────────────────────────────────

struct Inner<T> {
    queue: VecDeque<T>,
    waker: Option<Waker>,
    closed: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// NotifySender — sync send that wakes the async receiver
// ─────────────────────────────────────────────────────────────────────────────

pub struct NotifySender<T> {
    inner: Arc<Mutex<Inner<T>>>,
}

impl<T> Clone for NotifySender<T> {
    fn clone(&self) -> Self {
        NotifySender { inner: Arc::clone(&self.inner) }
    }
}

impl<T> NotifySender<T> {
    /// Synchronous send — pushes value and wakes the receiver.
    /// Returns `Err(SendError(value))` if the receiver has been dropped.
    pub fn send(&self, value: T) -> Result<(), SendError<T>> {
        let waker = {
            let mut inner = self.inner.lock().unwrap();
            if inner.closed {
                return Err(SendError(value));
            }
            inner.queue.push_back(value);
            inner.waker.take()
        };
        if let Some(w) = waker {
            w.wake();
        }
        Ok(())
    }
}

impl<T> Drop for NotifySender<T> {
    fn drop(&mut self) {
        // If this is the last sender, mark the channel as closed and wake
        // the receiver so it can observe the disconnect.
        if Arc::strong_count(&self.inner) <= 2 {
            // 2 = this sender + the receiver
            let waker = {
                let mut inner = self.inner.lock().unwrap();
                inner.closed = true;
                inner.waker.take()
            };
            if let Some(w) = waker {
                w.wake();
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AsyncReceiver
// ─────────────────────────────────────────────────────────────────────────────

pub struct AsyncReceiver<T> {
    inner: Arc<Mutex<Inner<T>>>,
}

impl<T> AsyncReceiver<T> {
    /// Returns a future that resolves to `Ok(value)` when a value is available,
    /// or `Err(())` when all senders are dropped and the queue is empty.
    pub fn recv(&self) -> RecvFuture<'_, T> {
        RecvFuture { receiver: self }
    }
}

impl<T> Drop for AsyncReceiver<T> {
    fn drop(&mut self) {
        let mut inner = self.inner.lock().unwrap();
        inner.closed = true;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RecvFuture
// ─────────────────────────────────────────────────────────────────────────────

pub struct RecvFuture<'a, T> {
    receiver: &'a AsyncReceiver<T>,
}

impl<T> Future for RecvFuture<'_, T> {
    type Output = Result<T, ()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inner = self.receiver.inner.lock().unwrap();
        if let Some(value) = inner.queue.pop_front() {
            return Poll::Ready(Ok(value));
        }
        if inner.closed {
            return Poll::Ready(Err(()));
        }
        let needs_update = inner.waker
            .as_ref()
            .is_none_or(|w| !w.will_wake(cx.waker()));
        if needs_update {
            inner.waker = Some(cx.waker().clone());
        }
        Poll::Pending
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Constructor
// ─────────────────────────────────────────────────────────────────────────────

/// Create an async MPSC channel. The sender is sync (suitable for use from
/// non-async code); the receiver is async.
pub fn async_channel<T>() -> (NotifySender<T>, AsyncReceiver<T>) {
    let inner = Arc::new(Mutex::new(Inner {
        queue: VecDeque::new(),
        waker: None,
        closed: false,
    }));
    (
        NotifySender { inner: Arc::clone(&inner) },
        AsyncReceiver { inner },
    )
}
