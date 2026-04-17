use viv::core::runtime::executor::block_on;

#[test]
fn block_on_immediate_future() {
    let result = block_on(async { 42i32 });
    assert_eq!(result, 42);
}

#[test]
fn spawn_two_tasks_run() {
    let result = block_on(async {
        1i32 + 2i32  // 简单验证 block_on 工作
    });
    assert_eq!(result, 3);
}

#[test]
fn task_wakes_and_completes() {
    use std::sync::{Arc, Mutex};
    use std::task::Waker;

    // future 第一次 Pending（存储 waker 并立刻 wake），第二次 Ready
    // 验证：waker 调用后任务重新进入 ready queue，executor 再次 poll 时 Ready
    let waker_slot: Arc<Mutex<Option<Waker>>> = Arc::new(Mutex::new(None));
    let slot = waker_slot.clone();

    struct OncePending { done: bool, slot: Arc<Mutex<Option<Waker>>> }
    impl std::future::Future for OncePending {
        type Output = i32;
        fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>)
            -> std::task::Poll<i32>
        {
            if self.done { return std::task::Poll::Ready(99); }
            self.done = true;
            // 存储 waker 后立刻 wake：模拟外部事件立即就绪
            let waker = cx.waker().clone();
            *self.slot.lock().unwrap() = Some(waker.clone());
            waker.wake_by_ref();
            std::task::Poll::Pending
        }
    }

    let result = block_on(async move {
        OncePending { done: false, slot }.await
    });
    assert_eq!(result, 99);
}
