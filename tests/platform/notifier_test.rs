use viv::core::platform::PlatformNotifier;

#[test]
fn notify_and_drain() {
    let notifier = PlatformNotifier::new().expect("create notifier");
    let h = notifier.handle();
    assert!(h >= 0);
    notifier.notify().expect("notify");
    notifier.drain().expect("drain");
    notifier.drain().expect("drain empty");
}
