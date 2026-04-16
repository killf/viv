use std::os::unix::io::RawFd;

// FFI declarations
unsafe extern "C" {
    fn epoll_create1(flags: i32) -> i32;
    fn epoll_ctl(epfd: i32, op: i32, fd: i32, event: *mut EpollEvent) -> i32;
    fn epoll_wait(epfd: i32, events: *mut EpollEvent, maxevents: i32, timeout: i32) -> i32;
    fn close(fd: i32) -> i32;
}

pub const EPOLL_CTL_ADD: i32 = 1;
pub const EPOLL_CTL_DEL: i32 = 2;
pub const EPOLLIN: u32 = 0x001;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct EpollEvent {
    pub events: u32,
    pub data: u64,
}

pub struct Epoll {
    fd: RawFd,
}

impl Epoll {
    pub fn new() -> crate::Result<Self> {
        let fd = unsafe { epoll_create1(0) };
        if fd < 0 {
            return Err(crate::Error::Terminal(
                "epoll_create1 failed".to_string(),
            ));
        }
        Ok(Epoll { fd })
    }

    pub fn add(&self, target_fd: RawFd, token: u64) -> crate::Result<()> {
        let mut event = EpollEvent {
            events: EPOLLIN,
            data: token,
        };
        let ret = unsafe { epoll_ctl(self.fd, EPOLL_CTL_ADD, target_fd, &mut event) };
        if ret < 0 {
            return Err(crate::Error::Terminal("epoll_ctl add failed".to_string()));
        }
        Ok(())
    }

    pub fn remove(&self, target_fd: RawFd) -> crate::Result<()> {
        let ret = unsafe { epoll_ctl(self.fd, EPOLL_CTL_DEL, target_fd, std::ptr::null_mut()) };
        if ret < 0 {
            return Err(crate::Error::Terminal("epoll_ctl del failed".to_string()));
        }
        Ok(())
    }

    pub fn wait(&self, timeout_ms: i32) -> crate::Result<Vec<u64>> {
        const MAX_EVENTS: usize = 64;
        let mut events = [EpollEvent { events: 0, data: 0 }; MAX_EVENTS];
        let n = unsafe {
            epoll_wait(
                self.fd,
                events.as_mut_ptr(),
                MAX_EVENTS as i32,
                timeout_ms,
            )
        };
        if n < 0 {
            return Err(crate::Error::Terminal("epoll_wait failed".to_string()));
        }
        let tokens = events[..n as usize].iter().map(|e| e.data).collect();
        Ok(tokens)
    }
}

impl Drop for Epoll {
    fn drop(&mut self) {
        unsafe { close(self.fd) };
    }
}
