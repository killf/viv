use std::time::{Duration, Instant};
use viv::core::runtime::executor::block_on;
use viv::core::runtime::timer::sleep;

#[test]
fn sleep_waits_at_least_duration() {
    let start = Instant::now();
    block_on(async {
        sleep(Duration::from_millis(50)).await;
    });
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(45),
        "elapsed too short: {:?}",
        elapsed
    );
}

#[test]
fn two_sleeps_sequential() {
    let start = Instant::now();
    block_on(async {
        sleep(Duration::from_millis(30)).await;
        sleep(Duration::from_millis(30)).await;
    });
    assert!(start.elapsed() >= Duration::from_millis(55));
}
