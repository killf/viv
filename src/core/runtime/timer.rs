use super::reactor::with_reactor;
use crate::core::platform::PlatformTimer;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

pub struct Sleep {
    duration: Duration,
    timer: Option<PlatformTimer>,
    token: Option<u64>,
}

impl Drop for Sleep {
    fn drop(&mut self) {
        if let Some(t) = self.token.take() {
            with_reactor(|r| r.remove(t)).ok();
        }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.token.is_some() {
            if let Some(t) = self.token.take() {
                with_reactor(|r| r.remove(t)).ok();
            }
            if let Some(ref timer) = self.timer {
                timer.consume().ok();
            }
            return Poll::Ready(());
        }

        let Ok(timer) = PlatformTimer::new(self.duration) else {
            cx.waker().wake_by_ref();
            return Poll::Pending;
        };
        let handle = timer.handle();
        match with_reactor(|r| r.register_readable(handle, cx.waker().clone())) {
            Ok(Ok(token)) => {
                self.timer = Some(timer);
                self.token = Some(token);
                Poll::Pending
            }
            _ => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }
}

pub fn sleep(duration: Duration) -> Sleep {
    Sleep { duration, timer: None, token: None }
}
