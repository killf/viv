use crate::Error;
use std::os::unix::io::RawFd;

// Linux x86_64 termios layout (60 bytes total)
#[repr(C)]
pub struct Termios {
    pub c_iflag: u32,
    pub c_oflag: u32,
    pub c_cflag: u32,
    pub c_lflag: u32,
    pub c_line: u8,
    pub c_cc: [u8; 32],
    pub c_ispeed: u32,
    pub c_ospeed: u32,
}

unsafe extern "C" {
    pub fn tcgetattr(fd: RawFd, termios: *mut Termios) -> i32;
    pub fn tcsetattr(fd: RawFd, optional_actions: i32, termios: *const Termios) -> i32;
}

// c_lflag flags
pub const ECHO: u32 = 0o10;
pub const ICANON: u32 = 0o2;
pub const ISIG: u32 = 0o1;
pub const IEXTEN: u32 = 0o100000;

// c_iflag flags
pub const IXON: u32 = 0o2000;
pub const ICRNL: u32 = 0o400;

// c_oflag flags
pub const OPOST: u32 = 0o1;

// tcsetattr actions
pub const TCSAFLUSH: i32 = 2;

// c_cc indices
pub const VMIN: usize = 6;
pub const VTIME: usize = 5;

pub fn apply_raw_flags(termios: &mut Termios) {
    termios.c_lflag &= !(ECHO | ICANON | ISIG | IEXTEN);
    termios.c_iflag &= !(IXON | ICRNL);
    termios.c_oflag &= !OPOST;
}

pub struct RawMode {
    fd: RawFd,
    original: Termios,
}

impl RawMode {
    pub fn enable(fd: RawFd) -> crate::Result<Self> {
        let mut original = Termios {
            c_iflag: 0,
            c_oflag: 0,
            c_cflag: 0,
            c_lflag: 0,
            c_line: 0,
            c_cc: [0; 32],
            c_ispeed: 0,
            c_ospeed: 0,
        };

        let ret = unsafe { tcgetattr(fd, &mut original) };
        if ret != 0 {
            return Err(Error::Terminal(format!(
                "tcgetattr failed with code {}",
                ret
            )));
        }

        let mut raw = Termios {
            c_iflag: original.c_iflag,
            c_oflag: original.c_oflag,
            c_cflag: original.c_cflag,
            c_lflag: original.c_lflag,
            c_line: original.c_line,
            c_cc: original.c_cc,
            c_ispeed: original.c_ispeed,
            c_ospeed: original.c_ospeed,
        };

        apply_raw_flags(&mut raw);
        raw.c_cc[VMIN] = 1;
        raw.c_cc[VTIME] = 0;

        let ret = unsafe { tcsetattr(fd, TCSAFLUSH, &raw) };
        if ret != 0 {
            return Err(Error::Terminal(format!(
                "tcsetattr failed with code {}",
                ret
            )));
        }

        Ok(RawMode { fd, original })
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        unsafe {
            tcsetattr(self.fd, TCSAFLUSH, &self.original);
        }
    }
}
