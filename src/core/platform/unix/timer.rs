use crate::core::platform::types::RawHandle;
use std::time::Duration;

unsafe extern "C" {
    fn timerfd_create(clockid: i32, flags: i32) -> i32;
    fn timerfd_settime(
        fd: i32,
        flags: i32,
        new_value: *const Itimerspec,
        old_value: *mut Itimerspec,
    ) -> i32;
    fn close(fd: i32) -> i32;
    fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
}

const CLOCK_MONOTONIC: i32 = 1;
const TFD_NONBLOCK: i32 = 0o4000;

#[repr(C)]
struct Timespec {
    tv_sec: i64,
    tv_nsec: i64,
}

#[repr(C)]
struct Itimerspec {
    it_interval: Timespec,
    it_value: Timespec,
}

pub struct UnixTimer {
    fd: i32,
}

impl UnixTimer {
    pub fn new(duration: Duration) -> crate::Result<Self> {
        let fd = unsafe { timerfd_create(CLOCK_MONOTONIC, TFD_NONBLOCK) };
        if fd < 0 {
            return Err(crate::Error::Io(std::io::Error::last_os_error()));
        }
        let spec = Itimerspec {
            it_interval: Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            },
            it_value: Timespec {
                tv_sec: duration.as_secs() as i64,
                tv_nsec: duration.subsec_nanos() as i64,
            },
        };
        let ret = unsafe { timerfd_settime(fd, 0, &spec, std::ptr::null_mut()) };
        if ret != 0 {
            unsafe { close(fd) };
            return Err(crate::Error::Io(std::io::Error::last_os_error()));
        }
        Ok(UnixTimer { fd })
    }

    pub fn handle(&self) -> RawHandle {
        self.fd
    }

    pub fn consume(&self) -> crate::Result<()> {
        let mut buf = [0u8; 8];
        unsafe { read(self.fd, buf.as_mut_ptr(), 8) };
        Ok(())
    }
}

impl Drop for UnixTimer {
    fn drop(&mut self) {
        unsafe { close(self.fd) };
    }
}
