use crate::core::platform;
use crate::core::runtime::reactor::with_reactor;
use std::future::Future;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, ToSocketAddrs, TcpStream as StdTcp};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

pub struct TcpStream {
    inner: StdTcp,
}

impl TcpStream {
    pub fn from_std(stream: StdTcp) -> crate::Result<Self> {
        stream.set_nonblocking(true)?;
        Ok(TcpStream { inner: stream })
    }

    pub fn raw_handle(&self) -> crate::core::platform::RawHandle {
        platform::tcp_raw_handle(&self.inner)
    }

    pub fn inner_mut(&mut self) -> &mut StdTcp {
        &mut self.inner
    }

    pub fn into_inner(self) -> StdTcp {
        self.inner
    }

    pub fn connect(host: &str, port: u16) -> ConnectFuture {
        ConnectFuture {
            host: host.to_string(),
            port,
            done: false,
        }
    }

    pub fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> ReadFuture<'a> {
        ReadFuture {
            stream: self,
            buf,
            token: None,
        }
    }

    pub fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> WriteFuture<'a> {
        WriteFuture {
            stream: self,
            buf,
            written: 0,
            token: None,
        }
    }
}

// ── ConnectFuture ─────────────────────────────────────────────────────────────

pub struct ConnectFuture {
    host: String,
    port: u16,
    done: bool,
}

impl Future for ConnectFuture {
    type Output = crate::Result<TcpStream>;
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if !self.done {
            self.done = true;
            let addrs: Vec<SocketAddr> =
                match (self.host.as_str(), self.port).to_socket_addrs() {
                    Ok(iter) => iter.collect(),
                    Err(e) => {
                        return Poll::Ready(Err(crate::Error::Io(std::io::Error::other(
                            e.to_string(),
                        ))))
                    }
                };
            for addr in addrs {
                match StdTcp::connect_timeout(&addr, Duration::from_secs(2)) {
                    Ok(stream) => return Poll::Ready(TcpStream::from_std(stream)),
                    Err(_) => continue,
                }
            }
            Poll::Ready(Err(crate::Error::Io(std::io::Error::other(
                "failed to connect to any resolved address",
            ))))
        } else {
            Poll::Ready(Err(crate::Error::Io(io::Error::other(
                "already connected",
            ))))
        }
    }
}

// ── ReadFuture ────────────────────────────────────────────────────────────────

pub struct ReadFuture<'a> {
    stream: &'a mut TcpStream,
    buf: &'a mut [u8],
    token: Option<u64>,
}

impl<'a> Future for ReadFuture<'a> {
    type Output = crate::Result<usize>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.as_mut().get_mut();

        if let Some(t) = this.token.take() {
            with_reactor(|r| r.remove(t)).ok();
        }

        match this.stream.inner.read(this.buf) {
            Ok(n) => Poll::Ready(Ok(n)),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                let fd = this.stream.raw_handle();
                match with_reactor(|r| r.register_readable(fd, cx.waker().clone())) {
                    Ok(Ok(token)) => {
                        this.token = Some(token);
                        Poll::Pending
                    }
                    Ok(Err(e)) => Poll::Ready(Err(e)),
                    Err(e) => Poll::Ready(Err(e)),
                }
            }
            Err(e) => Poll::Ready(Err(crate::Error::Io(e))),
        }
    }
}

impl Drop for ReadFuture<'_> {
    fn drop(&mut self) {
        if let Some(t) = self.token.take() {
            with_reactor(|r| r.remove(t)).ok();
        }
    }
}

// ── WriteFuture ───────────────────────────────────────────────────────────────

pub struct WriteFuture<'a> {
    stream: &'a mut TcpStream,
    buf: &'a [u8],
    written: usize,
    token: Option<u64>,
}

impl<'a> Future for WriteFuture<'a> {
    type Output = crate::Result<()>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.as_mut().get_mut();

        if let Some(t) = this.token.take() {
            with_reactor(|r| r.remove(t)).ok();
        }

        loop {
            if this.written == this.buf.len() {
                return Poll::Ready(Ok(()));
            }
            match this.stream.inner.write(&this.buf[this.written..]) {
                Ok(n) => this.written += n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    let fd = this.stream.raw_handle();
                    match with_reactor(|r| r.register_writable(fd, cx.waker().clone())) {
                        Ok(Ok(token)) => {
                            this.token = Some(token);
                            return Poll::Pending;
                        }
                        Ok(Err(e)) => return Poll::Ready(Err(e)),
                        Err(e) => return Poll::Ready(Err(e)),
                    }
                }
                Err(e) => return Poll::Ready(Err(crate::Error::Io(e))),
            }
        }
    }
}

impl Drop for WriteFuture<'_> {
    fn drop(&mut self) {
        if let Some(t) = self.token.take() {
            with_reactor(|r| r.remove(t)).ok();
        }
    }
}
