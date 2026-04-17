use std::thread;
use std::time::{Duration, Instant};
use viv::core::runtime::channel::async_channel;
use viv::core::runtime::executor::block_on;

#[test]
fn send_then_recv() {
    let (tx, rx) = async_channel::<i32>();
    tx.send(42).unwrap();
    let val = block_on(async move { rx.recv().await.unwrap() });
    assert_eq!(val, 42);
}

#[test]
fn multiple_sends() {
    let (tx, rx) = async_channel::<i32>();
    tx.send(1).unwrap();
    tx.send(2).unwrap();
    tx.send(3).unwrap();
    let vals = block_on(async move {
        let a = rx.recv().await.unwrap();
        let b = rx.recv().await.unwrap();
        let c = rx.recv().await.unwrap();
        (a, b, c)
    });
    assert_eq!(vals, (1, 2, 3));
}

#[test]
fn recv_wakes_on_send() {
    let (tx, rx) = async_channel::<&str>();
    let start = Instant::now();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        tx.send("hello").unwrap();
    });
    let val = block_on(async move { rx.recv().await.unwrap() });
    assert_eq!(val, "hello");
    // Should have waited ~50ms, not instant and not much longer
    let elapsed = start.elapsed();
    assert!(
        elapsed >= Duration::from_millis(40),
        "recv returned too soon: {:?}",
        elapsed
    );
}

#[test]
fn sender_drop_signals_closed() {
    let (tx, rx) = async_channel::<i32>();
    drop(tx);
    let result = block_on(async move { rx.recv().await });
    assert!(
        result.is_err(),
        "recv should return Err after sender is dropped"
    );
}
