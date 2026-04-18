use crate::core::platform::types::RawHandle;

// ── FFI declarations ────────────────────────────────────────────────────────

unsafe extern "C" {
    fn open(path: *const u8, flags: i32, ...) -> i32;
    fn close(fd: i32) -> i32;
    fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
    fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    fn ioctl(fd: i32, request: u64, ...) -> i32;
    fn tcgetattr(fd: i32, termios: *mut Termios) -> i32;
    fn tcsetattr(fd: i32, optional_actions: i32, termios: *const Termios) -> i32;
    fn pipe(pipefd: *mut [i32; 2]) -> i32;
    fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    fn sigaction(signum: i32, act: *const Sigaction, oldact: *mut Sigaction) -> i32;
    fn __errno_location() -> *mut i32;
}

// ── Constants ───────────────────────────────────────────────────────────────

const F_GETFL: i32 = 3;
const F_SETFL: i32 = 4;
const O_RDONLY: i32 = 0;
const O_NONBLOCK: i32 = 0o4000;
const O_CLOEXEC: i32 = 0o2000000;
const EAGAIN: i32 = 11;

// Termios c_lflag flags
const ECHO: u32 = 0o10;
const ICANON: u32 = 0o2;
const ISIG: u32 = 0o1;
const IEXTEN: u32 = 0o100000;

// Termios c_iflag flags
const IXON: u32 = 0o2000;
const ICRNL: u32 = 0o400;

// Termios c_oflag flags
const OPOST: u32 = 0o1;

// tcsetattr actions
const TCSAFLUSH: i32 = 2;

// c_cc indices
const VMIN: usize = 6;
const VTIME: usize = 5;

// ioctl
const TIOCGWINSZ: u64 = 0x5413;

// Signal
const SIGWINCH: i32 = 28;
const SA_RESTART: i32 = 0x10000000;

// ── Termios struct ──────────────────────────────────────────────────────────

#[repr(C)]
struct Termios {
    c_iflag: u32,
    c_oflag: u32,
    c_cflag: u32,
    c_lflag: u32,
    c_line: u8,
    c_cc: [u8; 32],
    c_ispeed: u32,
    c_ospeed: u32,
}

impl Termios {
    fn zeroed() -> Self {
        Termios {
            c_iflag: 0,
            c_oflag: 0,
            c_cflag: 0,
            c_lflag: 0,
            c_line: 0,
            c_cc: [0; 32],
            c_ispeed: 0,
            c_ospeed: 0,
        }
    }

    fn copy_from(&mut self, other: &Termios) {
        self.c_iflag = other.c_iflag;
        self.c_oflag = other.c_oflag;
        self.c_cflag = other.c_cflag;
        self.c_lflag = other.c_lflag;
        self.c_line = other.c_line;
        self.c_cc = other.c_cc;
        self.c_ispeed = other.c_ispeed;
        self.c_ospeed = other.c_ospeed;
    }
}

// ── Winsize struct ──────────────────────────────────────────────────────────

#[repr(C)]
struct Winsize {
    ws_row: u16,
    ws_col: u16,
    ws_xpixel: u16,
    ws_ypixel: u16,
}

// ── Sigaction struct ────────────────────────────────────────────────────────

#[repr(C)]
struct Sigaction {
    sa_handler: usize,
    sa_flags: u64,
    sa_restorer: usize,
    sa_mask: [u64; 16],
}

// ── UnixTerminal ────────────────────────────────────────────────────────────

pub struct UnixTerminal {
    input_fd: i32,
    owns_input: bool,
    original_termios: Option<Termios>,
}

impl UnixTerminal {
    pub fn new() -> crate::Result<Self> {
        // Try /dev/tty first for an isolated file description
        let path = b"/dev/tty\0";
        let fd = unsafe { open(path.as_ptr(), O_RDONLY | O_CLOEXEC) };
        if fd >= 0 {
            // Set non-blocking on our owned fd
            let flags = unsafe { fcntl(fd, F_GETFL) };
            if flags < 0 {
                unsafe { close(fd) };
                return Err(crate::Error::Terminal(
                    "fcntl(F_GETFL) on /dev/tty failed".to_string(),
                ));
            }
            let ret = unsafe { fcntl(fd, F_SETFL, flags | O_NONBLOCK) };
            if ret < 0 {
                unsafe { close(fd) };
                return Err(crate::Error::Terminal(
                    "fcntl(F_SETFL) on /dev/tty failed".to_string(),
                ));
            }
            Ok(UnixTerminal {
                input_fd: fd,
                owns_input: true,
                original_termios: None,
            })
        } else {
            // Fall back to fd 0 -- do NOT set O_NONBLOCK on shared fd
            Ok(UnixTerminal {
                input_fd: 0,
                owns_input: false,
                original_termios: None,
            })
        }
    }

    pub fn enable_raw_mode(&mut self) -> crate::Result<()> {
        if self.original_termios.is_some() {
            return Ok(()); // already in raw mode
        }
        let mut original = Termios::zeroed();
        let ret = unsafe { tcgetattr(self.input_fd, &mut original) };
        if ret != 0 {
            return Err(crate::Error::Terminal(format!(
                "tcgetattr failed with code {}",
                ret
            )));
        }

        let mut raw = Termios::zeroed();
        raw.copy_from(&original);

        // Apply raw flags
        raw.c_lflag &= !(ECHO | ICANON | ISIG | IEXTEN);
        raw.c_iflag &= !(IXON | ICRNL);
        raw.c_oflag &= !OPOST;
        raw.c_cc[VMIN] = 1;
        raw.c_cc[VTIME] = 0;

        let ret = unsafe { tcsetattr(self.input_fd, TCSAFLUSH, &raw) };
        if ret != 0 {
            return Err(crate::Error::Terminal(format!(
                "tcsetattr failed with code {}",
                ret
            )));
        }
        self.original_termios = Some(original);
        Ok(())
    }

    pub fn disable_raw_mode(&mut self) -> crate::Result<()> {
        if let Some(ref original) = self.original_termios {
            let ret = unsafe { tcsetattr(self.input_fd, TCSAFLUSH, original) };
            if ret != 0 {
                return Err(crate::Error::Terminal(format!(
                    "tcsetattr restore failed with code {}",
                    ret
                )));
            }
            self.original_termios = None;
        }
        Ok(())
    }

    pub fn size(&self) -> crate::Result<(u16, u16)> {
        let mut ws = Winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let ret = unsafe { ioctl(1, TIOCGWINSZ, &mut ws) };
        if ret == 0 && ws.ws_col > 0 && ws.ws_row > 0 {
            Ok((ws.ws_row, ws.ws_col))
        } else {
            Ok((24, 80)) // fallback
        }
    }

    pub fn input_handle(&self) -> RawHandle {
        self.input_fd
    }

    pub fn owns_input(&self) -> bool {
        self.owns_input
    }

    pub fn read_input(&self, buf: &mut [u8]) -> crate::Result<usize> {
        let n = unsafe { read(self.input_fd, buf.as_mut_ptr(), buf.len()) };
        if n > 0 {
            Ok(n as usize)
        } else if n == 0 {
            Ok(0) // EOF
        } else {
            let errno = unsafe { *__errno_location() };
            if errno == EAGAIN {
                Ok(0) // would block
            } else {
                Err(crate::Error::Io(std::io::Error::from_raw_os_error(errno)))
            }
        }
    }
}

impl Drop for UnixTerminal {
    fn drop(&mut self) {
        // Restore terminal settings if still in raw mode
        let _ = self.disable_raw_mode();
        if self.owns_input {
            unsafe { close(self.input_fd) };
        }
    }
}

// ── UnixResizeListener ──────────────────────────────────────────────────────

// Global write end of the self-pipe for SIGWINCH handler
static mut RESIZE_SIGNAL_WRITE_FD: i32 = -1;

unsafe extern "C" fn sigwinch_handler(_sig: i32) {
    unsafe {
        let byte: u8 = 1;
        write(RESIZE_SIGNAL_WRITE_FD, &byte as *const u8, 1);
    }
}

pub struct UnixResizeListener {
    read_fd: i32,
    write_fd: i32,
}

impl UnixResizeListener {
    pub fn new() -> crate::Result<Self> {
        // Create the self-pipe
        let mut fds = [-1i32; 2];
        let ret = unsafe { pipe(&mut fds as *mut [i32; 2]) };
        if ret != 0 {
            return Err(crate::Error::Terminal(format!("pipe() failed: {}", ret)));
        }
        let (read_fd, write_fd) = (fds[0], fds[1]);

        // Set both ends non-blocking
        for &fd in &[read_fd, write_fd] {
            let flags = unsafe { fcntl(fd, F_GETFL) };
            if flags < 0 {
                unsafe {
                    close(read_fd);
                    close(write_fd);
                }
                return Err(crate::Error::Terminal(
                    "fcntl(F_GETFL) failed on signal pipe".to_string(),
                ));
            }
            let ret = unsafe { fcntl(fd, F_SETFL, flags | O_NONBLOCK) };
            if ret < 0 {
                unsafe {
                    close(read_fd);
                    close(write_fd);
                }
                return Err(crate::Error::Terminal(
                    "fcntl(F_SETFL) failed on signal pipe".to_string(),
                ));
            }
        }

        // Set global write fd for the signal handler
        unsafe {
            RESIZE_SIGNAL_WRITE_FD = write_fd;
        }

        // Install SIGWINCH handler
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
            return Err(crate::Error::Terminal(format!(
                "sigaction() failed: {}",
                ret
            )));
        }

        Ok(UnixResizeListener { read_fd, write_fd })
    }

    pub fn handle(&self) -> RawHandle {
        self.read_fd
    }

    pub fn drain(&self) {
        let mut buf = [0u8; 64];
        loop {
            let n = unsafe { read(self.read_fd, buf.as_mut_ptr(), buf.len()) };
            if n <= 0 {
                break;
            }
        }
    }
}

impl Drop for UnixResizeListener {
    fn drop(&mut self) {
        unsafe {
            close(self.read_fd);
            close(self.write_fd);
        }
    }
}
