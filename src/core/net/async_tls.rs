use std::ffi::{CString, c_void};
use std::future::Future;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::core::runtime::reactor::reactor;
use super::async_tcp::AsyncTcpStream;

// OpenSSL FFI bindings (edition 2024 requires `unsafe extern "C"`)
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

// ── Helper futures ───────────────────────────────────────────────────────────

/// One-shot future: register fd readable with reactor, await wakeup.
pub async fn wait_readable(fd: RawFd) -> crate::Result<()> {
    WaitFd { fd, writable: false, registered: false }.await
}

/// One-shot future: register fd writable with reactor, await wakeup.
pub async fn wait_writable(fd: RawFd) -> crate::Result<()> {
    WaitFd { fd, writable: true, registered: false }.await
}

struct WaitFd {
    fd: RawFd,
    writable: bool,
    registered: bool,
}

impl Future for WaitFd {
    type Output = crate::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.registered {
            // Woken by reactor — fd is ready
            Poll::Ready(Ok(()))
        } else {
            // First poll: register with reactor and return Pending
            self.registered = true;
            let rc = reactor();
            let mut r = rc.lock().unwrap();
            if self.writable {
                r.register_writable(self.fd, cx.waker().clone());
            } else {
                r.register_readable(self.fd, cx.waker().clone());
            }
            Poll::Pending
        }
    }
}

// ── AsyncTlsStream ──────────────────────────────────────────────────────────

pub struct AsyncTlsStream {
    tcp: AsyncTcpStream,
    ssl: *mut c_void,
    ctx: *mut c_void,
}

unsafe impl Send for AsyncTlsStream {}

impl AsyncTlsStream {
    /// Connect to `host:port` over TLS with async handshake.
    pub async fn connect(host: &str, port: u16) -> crate::Result<Self> {
        // 1. Async TCP connect
        let tcp = AsyncTcpStream::connect(host, port).await?;
        let fd = tcp.raw_fd();

        // 2. Init OpenSSL
        unsafe {
            OPENSSL_init_ssl(0, std::ptr::null());
        }

        // 3. Create SSL_CTX
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

        // 4. Set default verify paths + SSL_VERIFY_PEER
        unsafe {
            SSL_CTX_set_default_verify_paths(ctx);
            SSL_CTX_set_verify(ctx, SSL_VERIFY_PEER, std::ptr::null());
        }

        // 5. Create SSL, set fd
        let ssl = unsafe {
            let ssl = SSL_new(ctx);
            if ssl.is_null() {
                SSL_CTX_free(ctx);
                return Err(crate::Error::Tls("SSL_new failed".into()));
            }
            let ret = SSL_set_fd(ssl, fd);
            if ret != 1 {
                SSL_free(ssl);
                SSL_CTX_free(ctx);
                return Err(crate::Error::Tls("SSL_set_fd failed".into()));
            }
            ssl
        };

        // 6. Set SNI hostname (SSL_CTRL_SET_TLSEXT_HOSTNAME = 55)
        let host_cstr = CString::new(host)
            .map_err(|_| crate::Error::Tls("invalid hostname".into()))?;
        unsafe {
            SSL_ctrl(ssl, 55, 0, host_cstr.as_ptr() as *mut c_void);
        }

        // 7. Async SSL handshake
        loop {
            let ret = unsafe { SSL_connect(ssl) };
            if ret == 1 {
                break; // handshake complete
            }
            let err = unsafe { SSL_get_error(ssl, ret) };
            match err {
                SSL_ERROR_WANT_READ => {
                    wait_readable(fd).await?;
                }
                SSL_ERROR_WANT_WRITE => {
                    wait_writable(fd).await?;
                }
                _ => {
                    unsafe {
                        SSL_free(ssl);
                        SSL_CTX_free(ctx);
                    }
                    return Err(crate::Error::Tls(
                        format!("SSL_connect failed: error code {}", err),
                    ));
                }
            }
        }

        Ok(AsyncTlsStream { tcp, ssl, ctx })
    }

    /// Return the underlying raw file descriptor.
    pub fn raw_fd(&self) -> RawFd {
        self.tcp.raw_fd()
    }

    /// Async read into buffer. Returns number of bytes read (0 = EOF).
    pub async fn read(&mut self, buf: &mut [u8]) -> crate::Result<usize> {
        loop {
            let ret = unsafe {
                SSL_read(self.ssl, buf.as_mut_ptr() as *mut c_void, buf.len() as i32)
            };
            if ret > 0 {
                return Ok(ret as usize);
            }
            if ret == 0 {
                return Ok(0); // EOF / clean shutdown
            }
            let err = unsafe { SSL_get_error(self.ssl, ret) };
            match err {
                SSL_ERROR_WANT_READ => {
                    wait_readable(self.raw_fd()).await?;
                }
                SSL_ERROR_WANT_WRITE => {
                    wait_writable(self.raw_fd()).await?;
                }
                _ => {
                    return Err(crate::Error::Tls(
                        format!("SSL_read failed: error code {}", err),
                    ));
                }
            }
        }
    }

    /// Async write all bytes from buffer.
    pub async fn write_all(&mut self, buf: &[u8]) -> crate::Result<()> {
        let mut written = 0;
        while written < buf.len() {
            let remaining = &buf[written..];
            let ret = unsafe {
                SSL_write(
                    self.ssl,
                    remaining.as_ptr() as *const c_void,
                    remaining.len() as i32,
                )
            };
            if ret > 0 {
                written += ret as usize;
                continue;
            }
            let err = unsafe { SSL_get_error(self.ssl, ret) };
            match err {
                SSL_ERROR_WANT_READ => {
                    wait_readable(self.raw_fd()).await?;
                }
                SSL_ERROR_WANT_WRITE => {
                    wait_writable(self.raw_fd()).await?;
                }
                _ => {
                    return Err(crate::Error::Tls(
                        format!("SSL_write failed: error code {}", err),
                    ));
                }
            }
        }
        Ok(())
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
