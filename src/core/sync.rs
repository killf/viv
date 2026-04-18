//! Synchronization helpers that never panic.
//!
//! Rust's `Mutex::lock()` returns `PoisonError` when a thread panics while
//! holding the lock. The default usage `.lock().unwrap()` propagates that
//! panic across every other lock holder, which means one buggy task can
//! crash the entire agent. `lock_or_recover` treats poisoning as a hint
//! (log later if needed) and returns the inner guard so the process keeps
//! running.

use std::sync::{Mutex, MutexGuard};

/// Acquire a mutex guard, recovering from poisoning.
///
/// If another thread panicked while holding this lock, the data may be in
/// an inconsistent state, but for our use cases (caches, handler maps,
/// reactor state) recovering is preferable to cascading the panic.
pub fn lock_or_recover<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    match m.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    }
}
