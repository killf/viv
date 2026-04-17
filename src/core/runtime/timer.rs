use std::future::Future;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use super::reactor::reactor;

// ── timerfd Linux syscall FFI ─────────────────────────────────────────────────

#[repr(C)]
struct Timespec { tv_sec: i64, tv_nsec: i64 }

#[repr(C)]
struct Itimerspec {
    it_interval: Timespec,
    it_value: Timespec,
}

unsafe extern "C" {
    fn timerfd_create(clockid: i32, flags: i32) -> i32;
    fn timerfd_settime(fd: i32, flags: i32, new_value: *const Itimerspec, old_value: *mut Itimerspec) -> i32;
    fn close(fd: i32) -> i32;
}

const CLOCK_MONOTONIC: i32 = 1;
const TFD_NONBLOCK: i32 = 0o4000;  // O_NONBLOCK = 2048 on Linux

fn create_timer_fd(duration: Duration) -> RawFd {
    let fd = unsafe { timerfd_create(CLOCK_MONOTONIC, TFD_NONBLOCK) };
    assert!(fd >= 0, "timerfd_create failed: errno indicates failure");
    let spec = Itimerspec {
        it_interval: Timespec { tv_sec: 0, tv_nsec: 0 },
        it_value: Timespec {
            tv_sec: duration.as_secs() as i64,
            tv_nsec: duration.subsec_nanos() as i64,
        },
    };
    let ret = unsafe { timerfd_settime(fd, 0, &spec, std::ptr::null_mut()) };
    assert!(ret == 0, "timerfd_settime failed");
    fd
}

// ── Sleep Future ──────────────────────────────────────────────────────────────

pub struct Sleep {
    duration: Duration,
    fd: Option<RawFd>,
    token: Option<u64>,
    fired: bool,
}

impl Sleep {
    fn new(duration: Duration) -> Self {
        Sleep { duration, fd: None, token: None, fired: false }
    }
}

impl Drop for Sleep {
    fn drop(&mut self) {
        // 确保 timerfd 被关闭，即使 future 被 cancel
        if let Some(token) = self.token.take() {
            reactor().lock().unwrap().remove(token);
        }
        if let Some(fd) = self.fd.take() {
            unsafe { close(fd) };
        }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.fired {
            return Poll::Ready(());
        }

        if self.fd.is_none() {
            // 第一次 poll：创建 timerfd 并注册到 reactor
            let fd = create_timer_fd(self.duration);
            let token = reactor().lock().unwrap().register_readable(fd, cx.waker().clone());
            self.fd = Some(fd);
            self.token = Some(token);
            return Poll::Pending;
        }

        // reactor 唤醒后，timerfd 已就绪，标记完成
        self.fired = true;
        // token 和 fd 由 Drop 清理，或在此处提前清理
        if let Some(token) = self.token.take() {
            reactor().lock().unwrap().remove(token);
        }
        if let Some(fd) = self.fd.take() {
            unsafe { close(fd) };
        }
        Poll::Ready(())
    }
}

pub fn sleep(duration: Duration) -> Sleep {
    Sleep::new(duration)
}
