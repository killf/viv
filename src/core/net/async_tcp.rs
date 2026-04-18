use std::future::Future;
use std::io::{self, Read, Write};
use std::net::TcpStream;
#[cfg(unix)]
use std::os::unix::io::AsRawFd;
#[cfg(windows)]
use std::os::windows::io::AsRawSocket;
use std::pin::Pin;
use std::task::{Context, Poll};
use crate::core::runtime::reactor::reactor;
use super::tcp::connect as tcp_connect;

pub struct AsyncTcpStream {
    inner: TcpStream,
}

impl AsyncTcpStream {
    pub fn from_std(stream: TcpStream) -> Self {
        stream.set_nonblocking(true).expect("set_nonblocking");
        AsyncTcpStream { inner: stream }
    }

    pub fn raw_handle(&self) -> crate::core::platform::RawHandle {
        #[cfg(unix)]
        { self.inner.as_raw_fd() }
        #[cfg(windows)]
        { self.inner.as_raw_socket() as crate::core::platform::RawHandle }
    }

    pub fn inner_mut(&mut self) -> &mut TcpStream { &mut self.inner }

    pub fn connect(host: &str, port: u16) -> ConnectFuture {
        ConnectFuture { host: host.to_string(), port, done: false }
    }

    pub fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> ReadFuture<'a> {
        ReadFuture { stream: self, buf, token: None }
    }

    pub fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> WriteFuture<'a> {
        WriteFuture { stream: self, buf, written: 0, token: None }
    }
}

// ── ConnectFuture ─────────────────────────────────────────────────────────────

pub struct ConnectFuture { host: String, port: u16, done: bool }

impl Future for ConnectFuture {
    type Output = crate::Result<AsyncTcpStream>;
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if !self.done {
            self.done = true;
            match tcp_connect(&self.host, self.port) {
                Ok(stream) => Poll::Ready(Ok(AsyncTcpStream::from_std(stream))),
                Err(e) => Poll::Ready(Err(e)),
            }
        } else {
            Poll::Ready(Err(crate::Error::Io(
                io::Error::other("already connected"),
            )))
        }
    }
}

// ── ReadFuture ────────────────────────────────────────────────────────────────

pub struct ReadFuture<'a> {
    stream: &'a mut AsyncTcpStream,
    buf: &'a mut [u8],
    token: Option<u64>,
}

impl<'a> Future for ReadFuture<'a> {
    type Output = crate::Result<usize>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.as_mut().get_mut();

        // 清除上次注册（重新注册新 waker）
        if let Some(t) = this.token.take() {
            reactor().lock().unwrap().remove(t);
        }

        match this.stream.inner.read(this.buf) {
            Ok(n) => Poll::Ready(Ok(n)),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                let fd = this.stream.raw_handle();
                let token = reactor().lock().unwrap().register_readable(fd, cx.waker().clone());
                this.token = Some(token);
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(crate::Error::Io(e))),
        }
    }
}

impl Drop for ReadFuture<'_> {
    fn drop(&mut self) {
        if let Some(t) = self.token.take() {
            reactor().lock().unwrap().remove(t);
        }
    }
}

// ── WriteFuture ───────────────────────────────────────────────────────────────

pub struct WriteFuture<'a> {
    stream: &'a mut AsyncTcpStream,
    buf: &'a [u8],
    written: usize,
    token: Option<u64>,
}

impl<'a> Future for WriteFuture<'a> {
    type Output = crate::Result<()>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.as_mut().get_mut();

        if let Some(t) = this.token.take() {
            reactor().lock().unwrap().remove(t);
        }

        loop {
            if this.written == this.buf.len() {
                return Poll::Ready(Ok(()));
            }
            match this.stream.inner.write(&this.buf[this.written..]) {
                Ok(n) => this.written += n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    let fd = this.stream.raw_handle();
                    let token = reactor().lock().unwrap().register_writable(fd, cx.waker().clone());
                    this.token = Some(token);
                    return Poll::Pending;
                }
                Err(e) => return Poll::Ready(Err(crate::Error::Io(e))),
            }
        }
    }
}

impl Drop for WriteFuture<'_> {
    fn drop(&mut self) {
        if let Some(t) = self.token.take() {
            reactor().lock().unwrap().remove(t);
        }
    }
}
