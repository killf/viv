use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

enum MaybeDone<F: Future> {
    Pending(F),
    Done(F::Output),
    Taken,
}

impl<F: Future + Unpin> MaybeDone<F>
where
    F::Output: Unpin,
{
    fn poll(&mut self, cx: &mut Context<'_>) -> bool {
        match self {
            MaybeDone::Pending(f) => match Pin::new(f).poll(cx) {
                Poll::Ready(val) => {
                    *self = MaybeDone::Done(val);
                    true
                }
                Poll::Pending => false,
            },
            MaybeDone::Done(_) => true,
            MaybeDone::Taken => true,
        }
    }

    fn take(&mut self) -> F::Output {
        match std::mem::replace(self, MaybeDone::Taken) {
            MaybeDone::Done(v) => v,
            _ => panic!("MaybeDone::take called on non-Done variant"),
        }
    }
}

// --- join ---

pub struct Join<A: Future, B: Future> {
    a: MaybeDone<A>,
    b: MaybeDone<B>,
}

pub fn join<A, B>(a: A, b: B) -> Join<A, B>
where
    A: Future + Unpin,
    A::Output: Unpin,
    B: Future + Unpin,
    B::Output: Unpin,
{
    Join {
        a: MaybeDone::Pending(a),
        b: MaybeDone::Pending(b),
    }
}

impl<A, B> Future for Join<A, B>
where
    A: Future + Unpin,
    A::Output: Unpin,
    B: Future + Unpin,
    B::Output: Unpin,
{
    type Output = (A::Output, B::Output);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let a_done = this.a.poll(cx);
        let b_done = this.b.poll(cx);
        if a_done && b_done {
            Poll::Ready((this.a.take(), this.b.take()))
        } else {
            Poll::Pending
        }
    }
}

// --- join_all ---

pub struct JoinAll<F: Future> {
    futures: Vec<MaybeDone<F>>,
}

pub fn join_all<F>(futures: Vec<F>) -> JoinAll<F>
where
    F: Future + Unpin,
    F::Output: Unpin,
{
    JoinAll {
        futures: futures.into_iter().map(MaybeDone::Pending).collect(),
    }
}

impl<F> Future for JoinAll<F>
where
    F: Future + Unpin,
    F::Output: Unpin,
{
    type Output = Vec<F::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut all_done = true;
        for slot in this.futures.iter_mut() {
            if !slot.poll(cx) {
                all_done = false;
            }
        }
        if all_done {
            Poll::Ready(this.futures.iter_mut().map(|s| s.take()).collect())
        } else {
            Poll::Pending
        }
    }
}
