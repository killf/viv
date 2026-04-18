use std::sync::{Arc, Mutex};
use std::thread;

use viv::core::sync::lock_or_recover;

#[test]
fn lock_or_recover_returns_guard_on_healthy_mutex() {
    let m = Mutex::new(42);
    let g = lock_or_recover(&m);
    assert_eq!(*g, 42);
}

#[test]
fn lock_or_recover_recovers_poisoned_mutex() {
    let m = Arc::new(Mutex::new(vec![1, 2, 3]));
    let m2 = Arc::clone(&m);
    // Poison the mutex by panicking while holding it.
    let _ = thread::spawn(move || {
        let _g = m2.lock().unwrap();
        panic!("poison the mutex");
    })
    .join();
    assert!(m.lock().is_err(), "precondition: mutex should be poisoned");

    // Should not panic and should yield the recovered data.
    let g = lock_or_recover(&m);
    assert_eq!(*g, vec![1, 2, 3]);
}
