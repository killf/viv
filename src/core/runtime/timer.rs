use super::reactor::reactor;
use crate::core::platform::PlatformTimer;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

pub struct Sleep {
    duration: Duration,
    timer: Option<PlatformTimer>,
    token: Option<u64>,
    fired: bool,
}

impl Sleep {
    fn new(duration: Duration) -> Self {
        Sleep {
            duration,
            timer: None,
            token: None,
            fired: false,
        }
    }
}

impl Drop for Sleep {
    fn drop(&mut self) {
        if let Some(token) = self.token.take() {
            reactor().lock().unwrap().remove(token);
        }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.fired {
            return Poll::Ready(());
        }

        if self.timer.is_none() {
            let timer = PlatformTimer::new(self.duration).expect("create timer");
            let handle = timer.handle();
            let token = reactor()
                .lock()
                .unwrap()
                .register_readable(handle, cx.waker().clone());
            self.timer = Some(timer);
            self.token = Some(token);
            return Poll::Pending;
        }

        self.fired = true;
        if let Some(ref timer) = self.timer {
            timer.consume().ok();
        }
        if let Some(token) = self.token.take() {
            reactor().lock().unwrap().remove(token);
        }
        Poll::Ready(())
    }
}

pub fn sleep(duration: Duration) -> Sleep {
    Sleep::new(duration)
}
