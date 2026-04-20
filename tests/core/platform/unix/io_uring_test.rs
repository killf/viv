use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::task::{Wake, Waker};
use std::time::Duration;
use viv::core::platform::PlatformAsyncReactor;
use viv::core::platform::PlatformNotifier;

struct TestWake { woken: AtomicBool }
impl Wake for TestWake {
    fn wake(self: Arc<Self>) { self.woken.store(true, Ordering::SeqCst); }
}

#[test]
fn io_uring_creates_and_drops() {
    PlatformAsyncReactor::new().expect("io_uring reactor must initialize");
}

#[test]
fn io_uring_poll_timeout_no_events() {
    let mut r = PlatformAsyncReactor::new().expect("create");
    let count = r.poll(Duration::from_millis(10)).expect("poll");
    assert_eq!(count, 0, "no events registered → count must be 0");
}

#[test]
fn io_uring_register_and_wake() {
    let mut r = PlatformAsyncReactor::new().expect("create");
    let notifier = PlatformNotifier::new().expect("notifier");

    let wake = Arc::new(TestWake { woken: AtomicBool::new(false) });
    let waker = Waker::from(wake.clone());

    let token = r.register_read(notifier.handle(), waker).expect("register");
    notifier.notify().expect("notify");

    let count = r.poll(Duration::from_millis(200)).expect("poll");
    assert!(count > 0, "expected at least one event");
    assert!(wake.woken.load(Ordering::SeqCst), "waker must have been called");

    r.deregister(token).ok();
}

#[test]
fn io_uring_deregister_before_fire() {
    let mut r = PlatformAsyncReactor::new().expect("create");
    let notifier = PlatformNotifier::new().expect("notifier");
    let wake = Arc::new(TestWake { woken: AtomicBool::new(false) });
    let token = r.register_read(notifier.handle(), Waker::from(wake)).expect("register");
    r.deregister(token).ok();
    let count = r.poll(Duration::from_millis(10)).expect("poll after deregister");
    assert_eq!(count, 0);
}
