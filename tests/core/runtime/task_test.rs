use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc;
use std::task::Poll;
use std::task::{Context, RawWaker, RawWakerVTable, Waker};

use viv::core::runtime::task::{Task, oneshot};

// Noop waker for manual polling
fn noop_raw_waker() -> RawWaker {
    fn no_op(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker {
        noop_raw_waker()
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);
    RawWaker::new(std::ptr::null(), &VTABLE)
}

fn noop_waker() -> Waker {
    unsafe { Waker::from_raw(noop_raw_waker()) }
}

fn poll_once<F: Future + Unpin>(f: &mut F) -> Poll<F::Output> {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    Pin::new(f).poll(&mut cx)
}

#[test]
fn oneshot_ready_after_send() {
    let (tx, mut rx) = oneshot::<i32>();

    // Before send: Pending
    assert!(poll_once(&mut rx).is_pending());

    // Send value
    tx.send(42);

    // After send: Ready(42)
    match poll_once(&mut rx) {
        Poll::Ready(v) => assert_eq!(v, 42),
        Poll::Pending => panic!("expected Ready after send"),
    }
}

#[test]
fn waker_sends_task_id_on_wake() {
    let (sender, receiver) = mpsc::channel::<usize>();

    let task = Task::new(0, async { /* no-op future */ }, sender);

    let waker = task.waker();
    waker.wake_by_ref();

    let received_id = receiver.recv().expect("expected task id on channel");
    assert_eq!(received_id, 0);
}
