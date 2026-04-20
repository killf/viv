use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::task::{Wake, Waker};
use std::time::Duration;
use viv::core::platform::{PlatformNotifier, PlatformReactor};

struct TestWake {
    woken: AtomicBool,
}
impl Wake for TestWake {
    fn wake(self: Arc<Self>) {
        self.woken.store(true, Ordering::SeqCst);
    }
}

#[test]
fn register_and_wake() {
    let mut reactor = PlatformReactor::new().expect("create reactor");
    let notifier = PlatformNotifier::new().expect("create notifier");
    let wake = Arc::new(TestWake {
        woken: AtomicBool::new(false),
    });
    let waker = Waker::from(wake.clone());
    let token = reactor
        .register_read(notifier.handle(), waker)
        .expect("register");
    notifier.notify().expect("notify");
    let count = reactor.poll(Duration::from_millis(100)).expect("poll");
    assert!(count > 0);
    assert!(wake.woken.load(Ordering::SeqCst));
    reactor.deregister(token).ok();
}

#[test]
fn poll_timeout_no_events() {
    let mut reactor = PlatformReactor::new().expect("create reactor");
    let count = reactor.poll(Duration::from_millis(1)).expect("poll");
    assert_eq!(count, 0);
}
