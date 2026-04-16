use std::ffi::{CString, c_void};
use std::io::{self, Read, Write};
use std::os::unix::io::AsRawFd;

use crate::net::tcp;

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

pub struct TlsStream {
    pub ssl: *mut c_void,
    pub ctx: *mut c_void,
    pub _tcp: std::net::TcpStream,
}

unsafe impl Send for TlsStream {}

impl TlsStream {
    pub fn connect(host: &str, port: u16) -> crate::Result<Self> {
        // 1. Init OpenSSL
        unsafe {
            OPENSSL_init_ssl(0, std::ptr::null());
        }

        // 2. Create SSL_CTX with TLS_client_method
        let ctx = unsafe {
            let method = TLS_client_method();
            if method.is_null() {
                return Err(crate::error::Error::Tls("TLS_client_method failed".into()));
            }
            let ctx = SSL_CTX_new(method);
            if ctx.is_null() {
                return Err(crate::error::Error::Tls("SSL_CTX_new failed".into()));
            }
            ctx
        };

        // 3. Set default verify paths + SSL_VERIFY_PEER
        unsafe {
            SSL_CTX_set_default_verify_paths(ctx);
            SSL_CTX_set_verify(ctx, SSL_VERIFY_PEER, std::ptr::null());
        }

        // 4. Create SSL, set fd from TCP connection
        let tcp_stream = tcp::connect(host, port)?;
        let fd = tcp_stream.as_raw_fd();

        let ssl = unsafe {
            let ssl = SSL_new(ctx);
            if ssl.is_null() {
                SSL_CTX_free(ctx);
                return Err(crate::error::Error::Tls("SSL_new failed".into()));
            }
            let ret = SSL_set_fd(ssl, fd);
            if ret != 1 {
                SSL_free(ssl);
                SSL_CTX_free(ctx);
                return Err(crate::error::Error::Tls("SSL_set_fd failed".into()));
            }
            ssl
        };

        // 5. Set SNI hostname
        // SSL_set_tlsext_host_name is a macro: SSL_ctrl(ssl, 55, 0, name_ptr)
        // SSL_CTRL_SET_TLSEXT_HOSTNAME = 55, TLSEXT_NAMETYPE_host_name = 0
        let host_cstr = CString::new(host)
            .map_err(|_| crate::error::Error::Tls("invalid hostname".into()))?;
        unsafe {
            SSL_ctrl(ssl, 55, 0, host_cstr.as_ptr() as *mut c_void);
        }

        // 6. SSL_connect
        let ret = unsafe { SSL_connect(ssl) };
        if ret != 1 {
            let err = unsafe { SSL_get_error(ssl, ret) };
            unsafe {
                SSL_free(ssl);
                SSL_CTX_free(ctx);
            }
            return Err(crate::error::Error::Tls(format!("SSL_connect failed: error code {}", err)));
        }

        // 7. Return TlsStream
        Ok(TlsStream { ssl, ctx, _tcp: tcp_stream })
    }
}

impl Read for TlsStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let ret = unsafe {
            SSL_read(self.ssl, buf.as_mut_ptr() as *mut c_void, buf.len() as i32)
        };
        if ret < 0 {
            Err(io::Error::new(io::ErrorKind::Other, "SSL_read failed"))
        } else {
            Ok(ret as usize)
        }
    }
}

impl Write for TlsStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let ret = unsafe {
            SSL_write(self.ssl, buf.as_ptr() as *const c_void, buf.len() as i32)
        };
        if ret < 0 {
            Err(io::Error::new(io::ErrorKind::Other, "SSL_write failed"))
        } else {
            Ok(ret as usize)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for TlsStream {
    fn drop(&mut self) {
        unsafe {
            SSL_shutdown(self.ssl);
            SSL_free(self.ssl);
            SSL_CTX_free(self.ctx);
        }
    }
}
