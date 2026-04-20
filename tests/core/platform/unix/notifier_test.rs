use viv::core::platform::PlatformNotifier;

#[test]
fn notify_and_drain() {
    let notifier = PlatformNotifier::new().expect("create notifier");
    let _h = notifier.handle();
    // handle is valid if new() succeeded
    notifier.notify().expect("notify");
    notifier.drain().expect("drain");
    notifier.drain().expect("drain empty");
}
