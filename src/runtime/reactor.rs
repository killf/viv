use std::collections::HashMap;
use std::os::unix::io::RawFd;
use std::sync::{Arc, Mutex, OnceLock};
use std::task::Waker;
use std::time::Duration;
use crate::event::Epoll;

static REACTOR: OnceLock<Arc<Mutex<Reactor>>> = OnceLock::new();

pub fn reactor() -> Arc<Mutex<Reactor>> {
    REACTOR.get_or_init(|| Arc::new(Mutex::new(Reactor::new()))).clone()
}

pub struct Reactor {
    epoll: Epoll,
    wakers: HashMap<u64, Waker>,      // token → Waker
    token_to_fd: HashMap<u64, RawFd>, // token → fd（用于 remove 时调用 epoll.remove）
    next_token: u64,
}

impl Reactor {
    fn new() -> Self {
        Reactor {
            epoll: Epoll::new().expect("reactor epoll_create"),
            wakers: HashMap::new(),
            token_to_fd: HashMap::new(),
            next_token: 1,
        }
    }

    /// 注册 fd 可读事件，返回 token（后续用于 remove）
    pub fn register_readable(&mut self, fd: RawFd, waker: Waker) -> u64 {
        let token = self.next_token;
        self.next_token += 1;
        self.epoll.add(fd, token).expect("epoll add");
        self.wakers.insert(token, waker);
        self.token_to_fd.insert(token, fd);
        token
    }

    /// 注销 token（fd 不再等待）
    pub fn remove(&mut self, token: u64) {
        if let Some(&fd) = self.token_to_fd.get(&token) {
            self.epoll.remove(fd).ok();
        }
        self.wakers.remove(&token);
        self.token_to_fd.remove(&token);
    }

    /// 等待 I/O 事件，超时后返回。唤醒已就绪的 Waker。
    pub fn wait(&mut self, timeout: Duration) {
        let ms = timeout.as_millis().min(i32::MAX as u128) as i32;
        match self.epoll.wait(ms) {
            Ok(tokens) => {
                for token in tokens {
                    // 移除注册（one-shot：就绪后自动移除）
                    if let Some(&fd) = self.token_to_fd.get(&token) {
                        self.epoll.remove(fd).ok();
                        self.token_to_fd.remove(&token);
                    }
                    if let Some(waker) = self.wakers.remove(&token) {
                        waker.wake();
                    }
                }
            }
            Err(_) => {} // EINTR 或超时，忽略
        }
    }
}
