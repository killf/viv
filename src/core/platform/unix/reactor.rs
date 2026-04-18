use crate::core::platform::types::RawHandle;
use std::collections::HashMap;
use std::task::Waker;
use std::time::Duration;

unsafe extern "C" {
    fn epoll_create1(flags: i32) -> i32;
    fn epoll_ctl(epfd: i32, op: i32, fd: i32, event: *mut EpollEvent) -> i32;
    fn epoll_wait(epfd: i32, events: *mut EpollEvent, maxevents: i32, timeout: i32) -> i32;
    fn close(fd: i32) -> i32;
    fn __errno_location() -> *mut i32;
}

const EPOLL_CTL_ADD: i32 = 1;
const EPOLL_CTL_DEL: i32 = 2;
const EPOLLIN: u32 = 0x001;
const EPOLLOUT: u32 = 0x004;
const EINTR: i32 = 4;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct EpollEvent {
    events: u32,
    data: u64,
}

pub struct EpollReactor {
    epfd: i32,
    wakers: HashMap<u64, Waker>,
    token_to_fd: HashMap<u64, RawHandle>,
    next_token: u64,
}

impl EpollReactor {
    pub fn new() -> crate::Result<Self> {
        let epfd = unsafe { epoll_create1(0) };
        if epfd < 0 {
            return Err(crate::Error::Io(std::io::Error::last_os_error()));
        }
        Ok(EpollReactor {
            epfd,
            wakers: HashMap::new(),
            token_to_fd: HashMap::new(),
            next_token: 1,
        })
    }

    pub fn register_read(&mut self, handle: RawHandle, waker: Waker) -> crate::Result<u64> {
        let token = self.next_token;
        self.next_token += 1;
        let mut event = EpollEvent {
            events: EPOLLIN,
            data: token,
        };
        let ret = unsafe { epoll_ctl(self.epfd, EPOLL_CTL_ADD, handle, &mut event) };
        if ret < 0 {
            return Err(crate::Error::Io(std::io::Error::last_os_error()));
        }
        self.wakers.insert(token, waker);
        self.token_to_fd.insert(token, handle);
        Ok(token)
    }

    pub fn register_write(&mut self, handle: RawHandle, waker: Waker) -> crate::Result<u64> {
        let token = self.next_token;
        self.next_token += 1;
        let mut event = EpollEvent {
            events: EPOLLOUT,
            data: token,
        };
        let ret = unsafe { epoll_ctl(self.epfd, EPOLL_CTL_ADD, handle, &mut event) };
        if ret < 0 {
            return Err(crate::Error::Io(std::io::Error::last_os_error()));
        }
        self.wakers.insert(token, waker);
        self.token_to_fd.insert(token, handle);
        Ok(token)
    }

    pub fn deregister(&mut self, token: u64) -> crate::Result<()> {
        if let Some(&fd) = self.token_to_fd.get(&token) {
            unsafe { epoll_ctl(self.epfd, EPOLL_CTL_DEL, fd, std::ptr::null_mut()) };
        }
        self.wakers.remove(&token);
        self.token_to_fd.remove(&token);
        Ok(())
    }

    pub fn poll(&mut self, timeout: Duration) -> crate::Result<usize> {
        let ms = timeout.as_millis().min(i32::MAX as u128) as i32;
        const MAX_EVENTS: usize = 64;
        let mut events = [EpollEvent { events: 0, data: 0 }; MAX_EVENTS];
        let n = unsafe { epoll_wait(self.epfd, events.as_mut_ptr(), MAX_EVENTS as i32, ms) };
        if n < 0 {
            let errno = unsafe { *__errno_location() };
            if errno == EINTR {
                return Ok(0);
            }
            return Err(crate::Error::Io(std::io::Error::from_raw_os_error(errno)));
        }
        let count = n as usize;
        for event in &events[..count] {
            let token = event.data;
            if let Some(&fd) = self.token_to_fd.get(&token) {
                unsafe { epoll_ctl(self.epfd, EPOLL_CTL_DEL, fd, std::ptr::null_mut()) };
                self.token_to_fd.remove(&token);
            }
            if let Some(waker) = self.wakers.remove(&token) {
                waker.wake();
            }
        }
        Ok(count)
    }

    pub fn epoll_fd(&self) -> i32 {
        self.epfd
    }
}

impl Drop for EpollReactor {
    fn drop(&mut self) {
        unsafe { close(self.epfd) };
    }
}
