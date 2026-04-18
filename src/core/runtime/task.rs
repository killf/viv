use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

pub type TaskId = usize;

// ---------------------------------------------------------------------------
// oneshot channel
// ---------------------------------------------------------------------------

struct OneshotInner<T> {
    value: Option<T>,
    waker: Option<Waker>,
}

pub struct OneshotSender<T>(Arc<Mutex<OneshotInner<T>>>);
pub struct OneshotReceiver<T>(Arc<Mutex<OneshotInner<T>>>);

pub fn oneshot<T>() -> (OneshotSender<T>, OneshotReceiver<T>) {
    let inner = Arc::new(Mutex::new(OneshotInner {
        value: None,
        waker: None,
    }));
    (OneshotSender(Arc::clone(&inner)), OneshotReceiver(inner))
}

impl<T> OneshotSender<T> {
    pub fn send(self, value: T) {
        let waker = {
            let mut inner = self.0.lock().unwrap();
            inner.value = Some(value);
            inner.waker.take()
        }; // lock released here
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

impl<T: Unpin> Future for OneshotReceiver<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        let mut inner = self.0.lock().unwrap();
        if let Some(value) = inner.value.take() {
            Poll::Ready(value)
        } else {
            let needs_update = inner
                .waker
                .as_ref()
                .is_none_or(|w| !w.will_wake(cx.waker()));
            if needs_update {
                inner.waker = Some(cx.waker().clone());
            }
            Poll::Pending
        }
    }
}

// ---------------------------------------------------------------------------
// JoinHandle
// ---------------------------------------------------------------------------

pub struct JoinHandle<T>(pub OneshotReceiver<T>);

impl<T: Unpin> Future for JoinHandle<T> {
    type Output = T;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        Pin::new(&mut self.0).poll(cx)
    }
}

// ---------------------------------------------------------------------------
// Task
// ---------------------------------------------------------------------------

pub struct Task {
    pub id: TaskId,
    pub future: Mutex<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>,
    pub sender: Sender<TaskId>,
}

impl Task {
    pub fn new(
        id: TaskId,
        future: impl Future<Output = ()> + Send + 'static,
        sender: Sender<TaskId>,
    ) -> Arc<Self> {
        Arc::new(Task {
            id,
            future: Mutex::new(Box::pin(future)),
            sender,
        })
    }

    pub fn waker(self: &Arc<Self>) -> Waker {
        let ptr = Arc::into_raw(Arc::clone(self)) as *const ();
        unsafe { Waker::from_raw(RawWaker::new(ptr, &TASK_VTABLE)) }
    }
}

// ---------------------------------------------------------------------------
// RawWaker vtable for Task
// ---------------------------------------------------------------------------

static TASK_VTABLE: RawWakerVTable =
    RawWakerVTable::new(task_clone, task_wake, task_wake_by_ref, task_drop);

unsafe fn task_clone(ptr: *const ()) -> RawWaker {
    let arc = unsafe { Arc::from_raw(ptr as *const Task) };
    let clone = Arc::clone(&arc);
    // Don't drop the original — it still belongs to the existing Waker.
    mem::forget(arc);
    RawWaker::new(Arc::into_raw(clone) as *const (), &TASK_VTABLE)
}

unsafe fn task_wake(ptr: *const ()) {
    // Reconstruct Arc and let it drop at end of scope (consuming wake).
    let arc = unsafe { Arc::from_raw(ptr as *const Task) };
    let _ = arc.sender.send(arc.id);
    // arc drops here, decrementing refcount
}

unsafe fn task_wake_by_ref(ptr: *const ()) {
    // Reconstruct Arc but do NOT drop it — the Waker still owns the refcount.
    let arc = unsafe { Arc::from_raw(ptr as *const Task) };
    let _ = arc.sender.send(arc.id);
    mem::forget(arc);
}

unsafe fn task_drop(ptr: *const ()) {
    // Reconstruct Arc and drop it, decrementing the refcount.
    drop(unsafe { Arc::from_raw(ptr as *const Task) });
}
