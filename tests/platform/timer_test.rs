use viv::core::platform::{PlatformTimer, PlatformReactor};
use std::task::{Wake, Waker};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;

struct TestWake { woken: AtomicBool }
impl Wake for TestWake {
    fn wake(self: Arc<Self>) { self.woken.store(true, Ordering::SeqCst); }
}

#[test]
fn timer_fires_after_duration() {
    let timer = PlatformTimer::new(Duration::from_millis(50)).expect("create timer");
    let mut reactor = PlatformReactor::new().expect("create reactor");
    let wake = Arc::new(TestWake { woken: AtomicBool::new(false) });
    let waker = Waker::from(wake.clone());
    reactor.register_read(timer.handle(), waker).expect("register");
    reactor.poll(Duration::from_millis(200)).expect("poll");
    assert!(wake.woken.load(Ordering::SeqCst));
}
