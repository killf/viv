use super::reactor::reactor;
use crate::core::platform::PlatformTimer;
use crate::core::sync::lock_or_recover;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

pub struct Sleep {
    duration: Duration,
    start: Option<Instant>,
    timer: Option<PlatformTimer>,
    token: Option<u64>,
    fired: bool,
}

impl Sleep {
    fn new(duration: Duration) -> Self {
        Sleep {
            duration,
            start: None,
            timer: None,
            token: None,
            fired: false,
        }
    }
}

impl Drop for Sleep {
    fn drop(&mut self) {
        if let Some(token) = self.token.take() {
            let r = reactor();
            lock_or_recover(&r).remove(token);
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
            self.start.get_or_insert_with(Instant::now);
            match PlatformTimer::new(self.duration) {
                Ok(timer) => {
                    let handle = timer.handle();
                    let r = reactor();
                    let mut guard = lock_or_recover(&r);
                    match guard.register_readable(handle, cx.waker().clone()) {
                        Ok(token) => {
                            self.timer = Some(timer);
                            self.token = Some(token);
                            return Poll::Pending;
                        }
                        Err(_) => {
                            // Fall through to wall-clock fallback.
                        }
                    }
                }
                Err(_) => {
                    // Fall through to wall-clock fallback.
                }
            }

            // Fallback: no timer available. Check wall clock, yield if not
            // done yet, otherwise fire. This avoids a panic while preserving
            // approximate sleep semantics.
            let elapsed = self.start.map(|s| s.elapsed()).unwrap_or(Duration::ZERO);
            if elapsed >= self.duration {
                self.fired = true;
                return Poll::Ready(());
            }
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }

        self.fired = true;
        if let Some(ref timer) = self.timer {
            timer.consume().ok();
        }
        if let Some(token) = self.token.take() {
            let r = reactor();
            lock_or_recover(&r).remove(token);
        }
        Poll::Ready(())
    }
}

pub fn sleep(duration: Duration) -> Sleep {
    Sleep::new(duration)
}
