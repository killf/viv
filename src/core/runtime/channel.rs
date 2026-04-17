use std::cell::UnsafeCell;
use std::future::Future;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::task::{Context, Poll};

use super::reactor::reactor;

// ── FFI ──────────────────────────────────────────────────────────────────────

unsafe extern "C" {
    fn pipe(pipefd: *mut [RawFd; 2]) -> i32;
    fn close(fd: RawFd) -> i32;
    fn write(fd: RawFd, buf: *const u8, count: usize) -> isize;
    fn read(fd: RawFd, buf: *mut u8, count: usize) -> isize;
    fn fcntl(fd: RawFd, cmd: i32, ...) -> i32;
}

const F_GETFL: i32 = 3;
const F_SETFL: i32 = 4;
const O_NONBLOCK: i32 = 0o4000;

fn set_nonblocking(fd: RawFd) {
    unsafe {
        let flags = fcntl(fd, F_GETFL);
        fcntl(fd, F_SETFL, flags | O_NONBLOCK);
    }
}

// ── Shared pipe write fd (ref-counted) ───────────────────────────────────────

struct PipeWrite {
    fd: RawFd,
    ref_count: AtomicUsize,
}

impl PipeWrite {
    fn new(fd: RawFd) -> Arc<Self> {
        Arc::new(PipeWrite {
            fd,
            ref_count: AtomicUsize::new(1),
        })
    }

    fn notify(&self) {
        let byte: u8 = 1;
        unsafe { write(self.fd, &byte, 1) };
    }

    fn add_ref(&self) {
        self.ref_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Decrements ref count. Returns true if this was the last reference.
    fn dec_ref(&self) -> bool {
        self.ref_count.fetch_sub(1, Ordering::AcqRel) == 1
    }
}

// ── NotifySender ─────────────────────────────────────────────────────────────

pub struct NotifySender<T> {
    tx: mpsc::Sender<T>,
    pipe: Arc<PipeWrite>,
}

impl<T> Clone for NotifySender<T> {
    fn clone(&self) -> Self {
        self.pipe.add_ref();
        NotifySender {
            tx: self.tx.clone(),
            pipe: Arc::clone(&self.pipe),
        }
    }
}

impl<T> Drop for NotifySender<T> {
    fn drop(&mut self) {
        if self.pipe.dec_ref() {
            // Last sender dropped — wake the receiver so it sees Disconnected
            self.pipe.notify();
            unsafe { close(self.pipe.fd) };
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
        self.pipe.notify();
        Ok(())
    }
}

// ── AsyncReceiver ────────────────────────────────────────────────────────────

pub struct AsyncReceiver<T> {
    rx: mpsc::Receiver<T>,
    pipe_read: RawFd,
    /// Reactor registration token, mutated during poll via UnsafeCell.
    /// Safety: only one future polls this receiver at a time.
    token: UnsafeCell<Option<u64>>,
}

// Safety: AsyncReceiver is only used from one async task at a time.
// The UnsafeCell<Option<u64>> is only accessed during poll (single-threaded executor).
// mpsc::Receiver is not Sync, but we only access it from one task at a time.
unsafe impl<T: Send> Send for AsyncReceiver<T> {}
unsafe impl<T: Send> Sync for AsyncReceiver<T> {}

impl<T> Drop for AsyncReceiver<T> {
    fn drop(&mut self) {
        let token = self.token.get_mut();
        if let Some(t) = token.take() {
            reactor().lock().unwrap().remove(t);
        }
        unsafe { close(self.pipe_read) };
    }
}

impl<T> AsyncReceiver<T> {
    /// Returns a future that resolves to the next value from the channel.
    pub fn recv(&self) -> RecvFuture<'_, T> {
        RecvFuture { receiver: self }
    }

    /// Drain all notification bytes from the pipe (non-blocking).
    fn drain_pipe(&self) {
        let mut buf = [0u8; 64];
        loop {
            let n = unsafe { read(self.pipe_read, buf.as_mut_ptr(), buf.len()) };
            if n <= 0 {
                break;
            }
        }
    }
}

// ── RecvFuture ───────────────────────────────────────────────────────────────

pub struct RecvFuture<'a, T> {
    receiver: &'a AsyncReceiver<T>,
}

impl<T> Future for RecvFuture<'_, T> {
    type Output = crate::Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Drain notification bytes so the pipe is empty for next registration
        self.receiver.drain_pipe();

        match self.receiver.rx.try_recv() {
            Ok(value) => Poll::Ready(Ok(value)),
            Err(mpsc::TryRecvError::Disconnected) => Poll::Ready(Err(crate::Error::Io(
                std::io::Error::new(std::io::ErrorKind::BrokenPipe, "sender dropped"),
            ))),
            Err(mpsc::TryRecvError::Empty) => {
                // Register pipe_read as readable with reactor (one-shot epoll).
                let token = reactor()
                    .lock()
                    .unwrap()
                    .register_readable(self.receiver.pipe_read, cx.waker().clone());

                // Store token for cleanup in Drop.
                // Safety: only one future polls this receiver at a time.
                unsafe { *self.receiver.token.get() = Some(token) };

                Poll::Pending
            }
        }
    }
}

// ── Constructor ──────────────────────────────────────────────────────────────

/// Creates an async channel for bridging sync and async code.
///
/// `NotifySender` is `Clone` and can be used from any sync thread.
/// `AsyncReceiver` is used from an async context to receive values.
pub fn async_channel<T>() -> (NotifySender<T>, AsyncReceiver<T>) {
    let (tx, rx) = mpsc::channel();

    let mut pipefd = [0i32; 2];
    let ret = unsafe { pipe(&mut pipefd) };
    assert!(ret == 0, "pipe() failed");

    let pipe_read = pipefd[0];
    let pipe_write = pipefd[1];

    set_nonblocking(pipe_read);

    let pipe = PipeWrite::new(pipe_write);
    let sender = NotifySender { tx, pipe };
    let receiver = AsyncReceiver {
        rx,
        pipe_read,
        token: UnsafeCell::new(None),
    };

    (sender, receiver)
}
