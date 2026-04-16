use crate::Error;

// FFI declarations
unsafe extern "C" {
    pub fn pipe(pipefd: *mut [i32; 2]) -> i32;
    pub fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    pub fn sigaction(signum: i32, act: *const Sigaction, oldact: *mut Sigaction) -> i32;
    pub fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    pub fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
    pub fn close(fd: i32) -> i32;
}

// Signal constants
pub const SIGWINCH: i32 = 28;
pub const SA_RESTART: i32 = 0x10000000;

// fcntl commands
pub const F_GETFL: i32 = 3;
pub const F_SETFL: i32 = 4;

// open flags
pub const O_NONBLOCK: i32 = 0o4000;

// Linux x86_64 sigaction layout:
// sa_handler(8) + sa_flags(8) + sa_restorer(8) + sa_mask(128) = 152 bytes
#[repr(C)]
pub struct Sigaction {
    pub sa_handler: usize,
    pub sa_flags: u64,
    pub sa_restorer: usize,
    pub sa_mask: [u64; 16], // 128-byte sigset_t
}

// Global write end of the self-pipe, set before installing the signal handler
static mut SIGNAL_WRITE_FD: i32 = -1;

unsafe extern "C" fn sigwinch_handler(_sig: i32) {
    unsafe {
        let byte: u8 = 1;
        write(SIGNAL_WRITE_FD, &byte as *const u8, 1);
    }
}

pub struct SignalPipe {
    read_fd: i32,
    write_fd: i32,
}

impl SignalPipe {
    pub fn new() -> crate::Result<Self> {
        // 1. Create the pipe
        let mut fds = [-1i32; 2];
        let ret = unsafe { pipe(&mut fds as *mut [i32; 2]) };
        if ret != 0 {
            return Err(Error::Terminal(format!("pipe() failed: {}", ret)));
        }
        let (read_fd, write_fd) = (fds[0], fds[1]);

        // 2. Set both ends non-blocking
        let flags = unsafe { fcntl(read_fd, F_GETFL) };
        if flags < 0 {
            unsafe { close(read_fd); close(write_fd); }
            return Err(Error::Terminal(format!("fcntl(F_GETFL) failed on read_fd: {}", flags)));
        }
        let ret = unsafe { fcntl(read_fd, F_SETFL, flags | O_NONBLOCK) };
        if ret < 0 {
            unsafe { close(read_fd); close(write_fd); }
            return Err(Error::Terminal(format!("fcntl(F_SETFL) failed on read_fd: {}", ret)));
        }

        let flags = unsafe { fcntl(write_fd, F_GETFL) };
        if flags < 0 {
            unsafe { close(read_fd); close(write_fd); }
            return Err(Error::Terminal(format!("fcntl(F_GETFL) failed on write_fd: {}", flags)));
        }
        let ret = unsafe { fcntl(write_fd, F_SETFL, flags | O_NONBLOCK) };
        if ret < 0 {
            unsafe { close(read_fd); close(write_fd); }
            return Err(Error::Terminal(format!("fcntl(F_SETFL) failed on write_fd: {}", ret)));
        }

        // 3. Set global write fd
        unsafe {
            SIGNAL_WRITE_FD = write_fd;
        }

        // 4. Install sigaction for SIGWINCH
        let sa = Sigaction {
            sa_handler: sigwinch_handler as *const () as usize,
            sa_flags: SA_RESTART as u64,
            sa_restorer: 0,
            sa_mask: [0u64; 16],
        };
        let ret = unsafe { sigaction(SIGWINCH, &sa, std::ptr::null_mut()) };
        if ret != 0 {
            unsafe {
                close(read_fd);
                close(write_fd);
            }
            return Err(Error::Terminal(format!("sigaction() failed: {}", ret)));
        }

        Ok(SignalPipe { read_fd, write_fd })
    }

    /// Returns the read end of the pipe, suitable for epoll registration.
    pub fn read_fd(&self) -> i32 {
        self.read_fd
    }

    /// Drain all pending bytes from the read end; ignore EAGAIN (pipe empty).
    pub fn drain(&self) -> crate::Result<()> {
        let mut buf = [0u8; 64];
        loop {
            let n = unsafe { read(self.read_fd, buf.as_mut_ptr(), buf.len()) };
            if n > 0 {
                // consumed some bytes, keep draining
                continue;
            }
            if n == 0 {
                // EOF — shouldn't happen for a pipe with open write end, treat as done
                break;
            }
            // n < 0: check errno
            let errno = unsafe { *libc_errno() };
            if errno == EAGAIN || errno == EWOULDBLOCK {
                break; // pipe is empty, done
            }
            return Err(Error::Terminal(format!("read() on signal pipe failed: errno {}", errno)));
        }
        Ok(())
    }
}

impl Drop for SignalPipe {
    fn drop(&mut self) {
        unsafe {
            close(self.read_fd);
            close(self.write_fd);
        }
    }
}

// EAGAIN / EWOULDBLOCK errno values (Linux x86_64)
const EAGAIN: i32 = 11;
const EWOULDBLOCK: i32 = 11;

// Access errno via libc's __errno_location
unsafe fn libc_errno() -> *mut i32 {
    unsafe extern "C" {
        fn __errno_location() -> *mut i32;
    }
    unsafe { __errno_location() }
}
