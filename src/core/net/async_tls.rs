use std::ffi::{CString, c_void};
use std::future::Future;
use std::io;
use std::os::unix::io::AsRawFd;
use std::pin::Pin;
use std::task::{Context, Poll};
use crate::core::runtime::reactor::reactor;
use super::tcp;

// OpenSSL FFI bindings
unsafe extern "C" {
    fn OPENSSL_init_ssl(opts: u64, settings: *const c_void) -> i32;
    fn TLS_client_method() -> *mut c_void;
    fn SSL_CTX_new(method: *mut c_void) -> *mut c_void;
    fn SSL_CTX_free(ctx: *mut c_void);
    fn SSL_CTX_set_default_verify_paths(ctx: *mut c_void) -> i32;
    fn SSL_CTX_set_verify(ctx: *mut c_void, mode: i32, callback: *const c_void);
    fn SSL_new(ctx: *mut c_void) -> *mut c_void;
    fn SSL_free(ssl: *mut c_void);
    fn SSL_set_fd(ssl: *mut c_void, fd: i32) -> i32;
    fn SSL_ctrl(ssl: *mut c_void, cmd: i32, larg: i64, parg: *mut c_void) -> i64;
    fn SSL_connect(ssl: *mut c_void) -> i32;
    fn SSL_read(ssl: *mut c_void, buf: *mut c_void, num: i32) -> i32;
    fn SSL_write(ssl: *mut c_void, buf: *const c_void, num: i32) -> i32;
    fn SSL_shutdown(ssl: *mut c_void) -> i32;
    fn SSL_get_error(ssl: *mut c_void, ret: i32) -> i32;
}

const SSL_VERIFY_PEER: i32 = 0x01;
const SSL_ERROR_WANT_READ: i32 = 2;
const SSL_ERROR_WANT_WRITE: i32 = 3;

pub struct AsyncTlsStream {
    ssl: *mut c_void,
    ctx: *mut c_void,
    tcp: std::net::TcpStream,
}

unsafe impl Send for AsyncTlsStream {}

impl AsyncTlsStream {
    /// Establish TLS connection (blocking handshake, then set non-blocking for I/O).
    ///
    /// The TLS handshake is done in blocking mode to avoid the complexity of
    /// async handshake state machine. After the handshake completes, the socket
    /// is switched to non-blocking for async read/write operations.
    pub fn connect(host: &str, port: u16) -> ConnectFuture {
        ConnectFuture { host: host.to_string(), port, done: false }
    }

    pub fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> ReadFuture<'a> {
        ReadFuture { stream: self, buf, token: None }
    }

    pub fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> WriteAllFuture<'a> {
        WriteAllFuture { stream: self, buf, written: 0, token: None }
    }

    fn do_connect(host: &str, port: u16) -> crate::Result<Self> {
        // 1. Init OpenSSL
        unsafe { OPENSSL_init_ssl(0, std::ptr::null()); }

        // 2. Create SSL_CTX
        let ctx = unsafe {
            let method = TLS_client_method();
            if method.is_null() {
                return Err(crate::Error::Tls("TLS_client_method failed".into()));
            }
            let ctx = SSL_CTX_new(method);
            if ctx.is_null() {
                return Err(crate::Error::Tls("SSL_CTX_new failed".into()));
            }
            ctx
        };

        // 3. Set verify paths
        unsafe {
            SSL_CTX_set_default_verify_paths(ctx);
            SSL_CTX_set_verify(ctx, SSL_VERIFY_PEER, std::ptr::null());
        }

        // 4. TCP connect (blocking) + create SSL
        let tcp_stream = tcp::connect(host, port)?;
        let fd = tcp_stream.as_raw_fd();

        let ssl = unsafe {
            let ssl = SSL_new(ctx);
            if ssl.is_null() {
                SSL_CTX_free(ctx);
                return Err(crate::Error::Tls("SSL_new failed".into()));
            }
            if SSL_set_fd(ssl, fd) != 1 {
                SSL_free(ssl);
                SSL_CTX_free(ctx);
                return Err(crate::Error::Tls("SSL_set_fd failed".into()));
            }
            ssl
        };

        // 5. SNI hostname
        let host_cstr = CString::new(host)
            .map_err(|_| crate::Error::Tls("invalid hostname".into()))?;
        unsafe {
            SSL_ctrl(ssl, 55, 0, host_cstr.as_ptr() as *mut c_void);
        }

        // 6. TLS handshake (blocking)
        let ret = unsafe { SSL_connect(ssl) };
        if ret != 1 {
            let err = unsafe { SSL_get_error(ssl, ret) };
            unsafe {
                SSL_free(ssl);
                SSL_CTX_free(ctx);
            }
            return Err(crate::Error::Tls(format!("SSL_connect failed: error code {}", err)));
        }

        // 7. Switch to non-blocking for async I/O
        tcp_stream.set_nonblocking(true).map_err(|e| crate::Error::Io(e))?;

        Ok(AsyncTlsStream { ssl, ctx, tcp: tcp_stream })
    }
}

impl Drop for AsyncTlsStream {
    fn drop(&mut self) {
        unsafe {
            SSL_shutdown(self.ssl);
            SSL_free(self.ssl);
            SSL_CTX_free(self.ctx);
        }
    }
}

// ── ConnectFuture ────────────────────────────────────────────────────────────

pub struct ConnectFuture {
    host: String,
    port: u16,
    done: bool,
}

impl Future for ConnectFuture {
    type Output = crate::Result<AsyncTlsStream>;
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if !self.done {
            self.done = true;
            Poll::Ready(AsyncTlsStream::do_connect(&self.host, self.port))
        } else {
            Poll::Ready(Err(crate::Error::Io(
                io::Error::new(io::ErrorKind::Other, "already connected"),
            )))
        }
    }
}

// ── ReadFuture ───────────────────────────────────────────────────────────────

pub struct ReadFuture<'a> {
    stream: &'a mut AsyncTlsStream,
    buf: &'a mut [u8],
    token: Option<u64>,
}

impl<'a> Future for ReadFuture<'a> {
    type Output = crate::Result<usize>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.as_mut().get_mut();

        if let Some(t) = this.token.take() {
            reactor().lock().unwrap().remove(t);
        }

        let ret = unsafe {
            SSL_read(
                this.stream.ssl,
                this.buf.as_mut_ptr() as *mut c_void,
                this.buf.len() as i32,
            )
        };

        if ret > 0 {
            return Poll::Ready(Ok(ret as usize));
        }

        if ret == 0 {
            // Clean EOF
            return Poll::Ready(Ok(0));
        }

        let err = unsafe { SSL_get_error(this.stream.ssl, ret) };
        match err {
            SSL_ERROR_WANT_READ => {
                let fd = this.stream.tcp.as_raw_fd();
                let token = reactor().lock().unwrap().register_readable(fd, cx.waker().clone());
                this.token = Some(token);
                Poll::Pending
            }
            SSL_ERROR_WANT_WRITE => {
                // SSL sometimes needs to write during read (renegotiation)
                let fd = this.stream.tcp.as_raw_fd();
                let token = reactor().lock().unwrap().register_writable(fd, cx.waker().clone());
                this.token = Some(token);
                Poll::Pending
            }
            _ => Poll::Ready(Err(crate::Error::Tls(format!("SSL_read failed: error code {}", err)))),
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

// ── WriteAllFuture ───────────────────────────────────────────────────────────

pub struct WriteAllFuture<'a> {
    stream: &'a mut AsyncTlsStream,
    buf: &'a [u8],
    written: usize,
    token: Option<u64>,
}

impl<'a> Future for WriteAllFuture<'a> {
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

            let remaining = &this.buf[this.written..];
            let ret = unsafe {
                SSL_write(
                    this.stream.ssl,
                    remaining.as_ptr() as *const c_void,
                    remaining.len() as i32,
                )
            };

            if ret > 0 {
                this.written += ret as usize;
                continue;
            }

            let err = unsafe { SSL_get_error(this.stream.ssl, ret) };
            match err {
                SSL_ERROR_WANT_WRITE => {
                    let fd = this.stream.tcp.as_raw_fd();
                    let token = reactor().lock().unwrap().register_writable(fd, cx.waker().clone());
                    this.token = Some(token);
                    return Poll::Pending;
                }
                SSL_ERROR_WANT_READ => {
                    // SSL sometimes needs to read during write (renegotiation)
                    let fd = this.stream.tcp.as_raw_fd();
                    let token = reactor().lock().unwrap().register_readable(fd, cx.waker().clone());
                    this.token = Some(token);
                    return Poll::Pending;
                }
                _ => {
                    return Poll::Ready(Err(crate::Error::Tls(
                        format!("SSL_write failed: error code {}", err),
                    )));
                }
            }
        }
    }
}

impl Drop for WriteAllFuture<'_> {
    fn drop(&mut self) {
        if let Some(t) = self.token.take() {
            reactor().lock().unwrap().remove(t);
        }
    }
}
