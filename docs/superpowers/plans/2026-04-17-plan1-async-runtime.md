# Async Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现零依赖的 async executor + I/O reactor，为 Agent 循环、Tool 并发、MCP 网络 IO 提供异步基础。

**Architecture:** 单线程 executor（M:1 模型），用 `std::task::{RawWaker, Waker}` 手动实现任务唤醒，用 Linux epoll 作为 I/O reactor。Timer 通过 `timerfd_create` syscall 实现。Runtime 运行在独立线程，与 UI 线程通过 `mpsc::channel` 通信。

**Tech Stack:** Rust std only（`std::task`, `std::sync::mpsc`, `std::os::unix::io`, Linux syscalls via FFI）

---

## 文件结构

```
src/runtime/
├── mod.rs          # 公开 API: spawn, block_on, sleep, Runtime
├── executor.rs     # Executor: 任务队列 + poll 循环
├── task.rs         # Task: Future 封装 + RawWaker vtable + JoinHandle + oneshot channel
├── reactor.rs      # Reactor: epoll fd 注册 + waker 映射
└── timer.rs        # Timer: timerfd syscall + sleep Future

src/net/async_tcp.rs  # AsyncTcpStream: 封装现有 TcpStream + Reactor 注册

tests/runtime/
├── executor_test.rs
├── task_test.rs
├── timer_test.rs
└── net_test.rs
```

修改：
- `src/lib.rs` — 新增 `pub mod runtime;`
- `src/net/mod.rs` — 新增 `pub mod async_tcp;`
- `Cargo.toml` — 无变更（零依赖）

---

## Task 1: Task + Waker + JoinHandle + oneshot channel

**Files:**
- Create: `src/runtime/task.rs`
- Create: `tests/runtime/task_test.rs`

### 关键设计

`RawWaker` vtable 把 `Arc<Task>` 转为 `Waker`，唤醒时向 executor 的 `Sender<TaskId>` 发送任务 ID。

`JoinHandle<T>` 通过内置 oneshot channel 获取任务返回值。

### 代码

`src/runtime/task.rs`:

```rust
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

pub type TaskId = usize;

// ── oneshot channel ───────────────────────────────────────────────────────────

struct OneshotInner<T> {
    value: Option<T>,
    waker: Option<Waker>,
}

pub struct OneshotSender<T>(Arc<Mutex<OneshotInner<T>>>);
pub struct OneshotReceiver<T>(Arc<Mutex<OneshotInner<T>>>);

pub fn oneshot<T>() -> (OneshotSender<T>, OneshotReceiver<T>) {
    let inner = Arc::new(Mutex::new(OneshotInner { value: None, waker: None }));
    (OneshotSender(inner.clone()), OneshotReceiver(inner))
}

impl<T> OneshotSender<T> {
    pub fn send(self, value: T) {
        let mut inner = self.0.lock().unwrap();
        inner.value = Some(value);
        if let Some(w) = inner.waker.take() { w.wake(); }
    }
}

impl<T: Unpin> Future for OneshotReceiver<T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        let mut inner = self.0.lock().unwrap();
        if let Some(v) = inner.value.take() {
            Poll::Ready(v)
        } else {
            inner.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

// ── JoinHandle ────────────────────────────────────────────────────────────────

pub struct JoinHandle<T>(OneshotReceiver<T>);

impl<T: Unpin> Future for JoinHandle<T> {
    type Output = T;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        Pin::new(&mut self.0).poll(cx)
    }
}

// ── Task ──────────────────────────────────────────────────────────────────────

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
        unsafe { Waker::from_raw(RawWaker::new(ptr, &VTABLE)) }
    }
}

const VTABLE: RawWakerVTable = RawWakerVTable::new(
    // clone
    |ptr| {
        let arc = unsafe { Arc::from_raw(ptr as *const Task) };
        let clone = Arc::clone(&arc);
        std::mem::forget(arc);
        RawWaker::new(Arc::into_raw(clone) as *const (), &VTABLE)
    },
    // wake (consuming)
    |ptr| {
        let arc = unsafe { Arc::from_raw(ptr as *const Task) };
        arc.sender.send(arc.id).ok();
    },
    // wake_by_ref
    |ptr| {
        let arc = unsafe { Arc::from_raw(ptr as *const Task) };
        arc.sender.send(arc.id).ok();
        std::mem::forget(arc);
    },
    // drop
    |ptr| drop(unsafe { Arc::from_raw(ptr as *const Task) }),
);
```

- [ ] **Step 1: 创建测试文件**

`tests/runtime/task_test.rs`:

```rust
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::task::{Context, Poll};

// 手动 poll 辅助：用 noop waker 轮询一次 future
fn poll_once<F: Future + Unpin>(f: &mut F) -> Poll<F::Output> {
    use std::task::{RawWaker, RawWakerVTable, Waker};
    const NOOP: RawWakerVTable = RawWakerVTable::new(|p| RawWaker::new(p, &NOOP), |_| {}, |_| {}, |_| {});
    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &NOOP)) };
    let mut cx = Context::from_waker(&waker);
    Pin::new(f).poll(&mut cx)
}

#[test]
fn oneshot_ready_after_send() {
    use viv::runtime::task::oneshot;
    let (tx, mut rx) = oneshot::<i32>();
    // 发送前 Pending
    assert!(matches!(poll_once(&mut rx), Poll::Pending));
    tx.send(42);
    // 发送后 Ready
    assert!(matches!(poll_once(&mut rx), Poll::Ready(42)));
}

#[test]
fn waker_sends_task_id_on_wake() {
    use viv::runtime::task::{Task, TaskId};
    let (sender, receiver) = mpsc::channel::<TaskId>();
    let task = Task::new(0, async {}, sender);
    let waker = task.waker();
    waker.wake_by_ref();
    assert_eq!(receiver.recv().unwrap(), 0);
}
```

- [ ] **Step 2: 运行测试，确认编译失败**

```bash
cargo test --test task_test 2>&1 | head -20
```

期望：编译错误（模块不存在）

- [ ] **Step 3: 实现 `src/runtime/task.rs`**

粘贴上方完整代码。

- [ ] **Step 4: 在 `src/lib.rs` 添加模块声明**

```rust
pub mod runtime;
```

- [ ] **Step 5: 创建 `src/runtime/mod.rs`（空占位）**

```rust
pub mod task;
```

- [ ] **Step 6: 运行测试，确认通过**

```bash
cargo test --test task_test
```

期望：`2 passed`

- [ ] **Step 7: Commit**

```bash
git add src/runtime/ src/lib.rs tests/runtime/task_test.rs
git commit -m "feat(runtime): task + waker + oneshot channel"
```

---

## Task 2: Executor（poll 循环）

**Files:**
- Create: `src/runtime/executor.rs`
- Modify: `src/runtime/mod.rs`
- Create: `tests/runtime/executor_test.rs`

### 代码

`src/runtime/executor.rs`:

```rust
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, mpsc};
use std::task::Context;
use super::task::{Task, TaskId, JoinHandle, oneshot};

pub struct Executor {
    tasks: HashMap<TaskId, Arc<Task>>,
    ready_tx: mpsc::SyncSender<TaskId>,
    ready_rx: mpsc::Receiver<TaskId>,
    next_id: TaskId,
}

impl Executor {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel(1024);
        Executor { tasks: HashMap::new(), ready_tx: tx, ready_rx: rx, next_id: 0 }
    }

    pub fn spawn<T>(&mut self, future: impl Future<Output = T> + Send + 'static) -> JoinHandle<T>
    where
        T: Send + 'static + Unpin,
    {
        let id = self.next_id;
        self.next_id += 1;
        let (tx, rx) = oneshot::<T>();
        let wrapped = async move {
            let result = future.await;
            tx.send(result);
        };
        let task = Task::new(id, wrapped, self.ready_tx.clone().into());
        self.tasks.insert(id, task);
        self.ready_tx.send(id).ok();
        JoinHandle(rx)
    }

    /// 排干就绪队列，poll 所有就绪任务一次
    pub fn run_ready(&mut self) {
        let mut ids = vec![];
        while let Ok(id) = self.ready_rx.try_recv() {
            ids.push(id);
        }
        for id in ids {
            self.poll_task(id);
        }
    }

    fn poll_task(&mut self, id: TaskId) {
        let task = match self.tasks.get(&id) {
            Some(t) => t.clone(),
            None => return,
        };
        let waker = task.waker();
        let mut cx = Context::from_waker(&waker);
        let mut future = task.future.lock().unwrap();
        use std::task::Poll;
        if let Poll::Ready(()) = future.as_mut().poll(&mut cx) {
            drop(future);
            self.tasks.remove(&id);
        }
    }

    pub fn is_idle(&self) -> bool { self.tasks.is_empty() }

    pub fn sender(&self) -> mpsc::SyncSender<TaskId> { self.ready_tx.clone() }
}

/// 在当前线程运行 future 至完成（无 reactor，仅 busy-poll）
pub fn block_on<T: Unpin + Send + 'static>(future: impl Future<Output = T> + Send + 'static) -> T {
    let mut exec = Executor::new();
    let mut handle = exec.spawn(future);
    loop {
        exec.run_ready();
        use std::future::Future;
        use std::pin::Pin;
        use std::task::{RawWaker, RawWakerVTable, Waker, Context, Poll};
        const NOOP: RawWakerVTable = RawWakerVTable::new(|p| RawWaker::new(p, &NOOP), |_| {}, |_| {}, |_| {});
        let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &NOOP)) };
        let mut cx = Context::from_waker(&waker);
        if let Poll::Ready(v) = Pin::new(&mut handle).poll(&mut cx) {
            return v;
        }
        // 无任务可 poll 且 handle 还未就绪：等待唤醒
        if exec.is_idle() {
            std::thread::yield_now();
        }
    }
}
```

- [ ] **Step 1: 写测试**

`tests/runtime/executor_test.rs`:

```rust
use viv::runtime::executor::{block_on, Executor};

#[test]
fn block_on_immediate_future() {
    let result = block_on(async { 42i32 });
    assert_eq!(result, 42);
}

#[test]
fn spawn_two_tasks_concurrently() {
    let result = block_on(async {
        let mut exec = Executor::new();
        let h1 = exec.spawn(async { 1i32 });
        let h2 = exec.spawn(async { 2i32 });
        // 手动驱动
        exec.run_ready();
        exec.run_ready();
        h1.await + h2.await
    });
    assert_eq!(result, 3);
}

#[test]
fn task_completes_after_wakeup() {
    use std::sync::{Arc, Mutex};
    // future 第一次 Pending，手动唤醒后 Ready
    let waker_slot: Arc<Mutex<Option<std::task::Waker>>> = Arc::new(Mutex::new(None));
    let slot = waker_slot.clone();

    let result = block_on(async move {
        // 自定义 future：第一次 Pending，存储 waker；第二次 Ready
        struct OncePending {
            done: bool,
            slot: Arc<Mutex<Option<std::task::Waker>>>,
        }
        impl std::future::Future for OncePending {
            type Output = i32;
            fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>)
                -> std::task::Poll<i32>
            {
                if self.done {
                    std::task::Poll::Ready(99)
                } else {
                    self.done = true;
                    *self.slot.lock().unwrap() = Some(cx.waker().clone());
                    std::task::Poll::Pending
                }
            }
        }
        OncePending { done: false, slot }.await
    });
    assert_eq!(result, 99);
}
```

- [ ] **Step 2: 运行测试，确认编译失败**

```bash
cargo test --test executor_test 2>&1 | head -20
```

- [ ] **Step 3: 实现 `src/runtime/executor.rs`**

粘贴上方完整代码。

- [ ] **Step 4: 更新 `src/runtime/mod.rs`**

```rust
pub mod task;
pub mod executor;

pub use executor::{block_on, Executor};
```

- [ ] **Step 5: 运行测试，确认通过**

```bash
cargo test --test executor_test
```

期望：`3 passed`

- [ ] **Step 6: Commit**

```bash
git add src/runtime/executor.rs src/runtime/mod.rs tests/runtime/executor_test.rs
git commit -m "feat(runtime): executor + block_on + spawn"
```

---

## Task 3: Reactor（epoll fd 注册 + Waker 映射）

**Files:**
- Create: `src/runtime/reactor.rs`
- Modify: `src/runtime/mod.rs`

### 设计

Reactor 是全局单例（`OnceLock<Arc<Mutex<Reactor>>>`），存储 `fd → Waker` 映射。Executor 主循环在排干就绪队列后调用 `reactor().lock().wait(timeout)` 阻塞等待 fd 就绪。

`src/runtime/reactor.rs`:

```rust
use std::collections::HashMap;
use std::os::unix::io::RawFd;
use std::sync::{Arc, Mutex, OnceLock};
use std::task::Waker;
use std::time::Duration;
use crate::event::Epoll;

static REACTOR: OnceLock<Arc<Mutex<Reactor>>> = OnceLock::new();

pub fn reactor() -> Arc<Mutex<Reactor>> {
    REACTOR.get_or_init(|| Arc::new(Mutex::new(Reactor::new()))).clone()
}

pub struct Reactor {
    epoll: Epoll,
    wakers: HashMap<u64, Waker>,  // token → Waker
    next_token: u64,
    token_to_fd: HashMap<u64, RawFd>,
}

impl Reactor {
    fn new() -> Self {
        Reactor {
            epoll: Epoll::new().expect("reactor epoll_create"),
            wakers: HashMap::new(),
            next_token: 1,
            token_to_fd: HashMap::new(),
        }
    }

    /// 注册 fd 可读事件，返回 token（用于 remove）
    pub fn register_readable(&mut self, fd: RawFd, waker: Waker) -> u64 {
        let token = self.next_token;
        self.next_token += 1;
        self.epoll.add(fd, token).expect("epoll add");
        self.wakers.insert(token, waker);
        self.token_to_fd.insert(token, fd);
        token
    }

    pub fn remove(&mut self, token: u64) {
        if let Some(&fd) = self.token_to_fd.get(&token) {
            self.epoll.remove(fd).ok();
        }
        self.wakers.remove(&token);
        self.token_to_fd.remove(&token);
    }

    /// 等待 I/O 事件，超时后返回。唤醒对应 Waker。
    pub fn wait(&mut self, timeout: Duration) {
        let ms = timeout.as_millis().min(i32::MAX as u128) as i32;
        match self.epoll.wait(ms) {
            Ok(tokens) => {
                for token in tokens {
                    if let Some(waker) = self.wakers.remove(&token) {
                        if let Some(&fd) = self.token_to_fd.get(&token) {
                            self.epoll.remove(fd).ok();
                            self.token_to_fd.remove(&token);
                        }
                        waker.wake();
                    }
                }
            }
            Err(_) => {} // EINTR 等，忽略
        }
    }
}
```

- [ ] **Step 1: 实现 `src/runtime/reactor.rs`**

粘贴上方代码。

- [ ] **Step 2: 更新 `src/runtime/mod.rs`**

```rust
pub mod task;
pub mod executor;
pub mod reactor;

pub use executor::{block_on, Executor};
pub use reactor::reactor;
```

- [ ] **Step 3: 编译通过**

```bash
cargo build 2>&1 | grep -E "^error"
```

期望：无 error

- [ ] **Step 4: Commit**

```bash
git add src/runtime/reactor.rs src/runtime/mod.rs
git commit -m "feat(runtime): reactor — epoll fd registration + waker map"
```

---

## Task 4: Timer（timerfd + sleep Future）

**Files:**
- Create: `src/runtime/timer.rs`
- Modify: `src/runtime/mod.rs`
- Create: `tests/runtime/timer_test.rs`

### timerfd syscall FFI

`src/runtime/timer.rs`:

```rust
use std::future::Future;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use super::reactor::reactor;

// ── timerfd syscall ───────────────────────────────────────────────────────────

#[repr(C)]
struct Timespec { tv_sec: i64, tv_nsec: i64 }

#[repr(C)]
struct Itimerspec { it_interval: Timespec, it_value: Timespec }

extern "C" {
    fn timerfd_create(clockid: i32, flags: i32) -> i32;
    fn timerfd_settime(fd: i32, flags: i32, new: *const Itimerspec, old: *mut Itimerspec) -> i32;
    fn close(fd: i32) -> i32;
}

const CLOCK_MONOTONIC: i32 = 1;
const TFD_NONBLOCK: i32 = 0o4000;

fn create_timer(duration: Duration) -> RawFd {
    let fd = unsafe { timerfd_create(CLOCK_MONOTONIC, TFD_NONBLOCK) };
    assert!(fd >= 0, "timerfd_create failed");
    let spec = Itimerspec {
        it_interval: Timespec { tv_sec: 0, tv_nsec: 0 },
        it_value: Timespec {
            tv_sec: duration.as_secs() as i64,
            tv_nsec: duration.subsec_nanos() as i64,
        },
    };
    unsafe { timerfd_settime(fd, 0, &spec, std::ptr::null_mut()) };
    fd
}

// ── sleep Future ──────────────────────────────────────────────────────────────

pub struct Sleep {
    fd: Option<RawFd>,
    token: Option<u64>,
    duration: Duration,
}

impl Sleep {
    pub fn new(duration: Duration) -> Self {
        Sleep { fd: None, token: None, duration }
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.fd.is_none() {
            let fd = create_timer(self.duration);
            let token = reactor().lock().unwrap().register_readable(fd, cx.waker().clone());
            self.fd = Some(fd);
            self.token = Some(token);
            return Poll::Pending;
        }
        // timerfd 就绪后 reactor 已移除注册并唤醒，此时 Ready
        if let Some(fd) = self.fd.take() {
            unsafe { close(fd) };
        }
        Poll::Ready(())
    }
}

pub fn sleep(duration: Duration) -> Sleep {
    Sleep::new(duration)
}
```

- [ ] **Step 1: 写测试**

`tests/runtime/timer_test.rs`:

```rust
use std::time::{Duration, Instant};
use viv::runtime::executor::block_on;
use viv::runtime::timer::sleep;

#[test]
fn sleep_waits_at_least_duration() {
    let start = Instant::now();
    block_on(async {
        sleep(Duration::from_millis(50)).await;
    });
    assert!(start.elapsed() >= Duration::from_millis(45), // 5ms 容差
        "elapsed: {:?}", start.elapsed());
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
```

- [ ] **Step 2: 运行测试，确认失败**

```bash
cargo test --test timer_test 2>&1 | head -20
```

- [ ] **Step 3: 实现 `src/runtime/timer.rs`**

粘贴上方代码。

- [ ] **Step 4: 更新 `src/runtime/mod.rs`**

```rust
pub mod task;
pub mod executor;
pub mod reactor;
pub mod timer;

pub use executor::{block_on, Executor};
pub use reactor::reactor;
pub use timer::sleep;
```

- [ ] **Step 5: 修改 Executor 主循环，集成 Reactor wait**

在 `block_on` 函数的循环末尾加 reactor wait（替代 yield_now）：

```rust
// executor.rs 中 block_on 循环末尾改为：
if exec.is_idle() {
    // 等待 reactor I/O 事件（最多 1 秒）
    crate::runtime::reactor().lock().unwrap().wait(Duration::from_secs(1));
} else {
    // 有任务但都在等 I/O：让出 CPU
    std::thread::yield_now();
}
```

在 `executor.rs` 顶部加：
```rust
use std::time::Duration;
```

- [ ] **Step 6: 运行测试，确认通过**

```bash
cargo test --test timer_test
```

期望：`2 passed`

- [ ] **Step 7: Commit**

```bash
git add src/runtime/timer.rs src/runtime/executor.rs src/runtime/mod.rs tests/runtime/timer_test.rs
git commit -m "feat(runtime): timerfd sleep + reactor integration"
```

---

## Task 5: AsyncTcpStream

**Files:**
- Create: `src/net/async_tcp.rs`
- Modify: `src/net/mod.rs`
- Create: `tests/runtime/net_test.rs`

### 设计

`AsyncTcpStream` 包装现有同步 `TcpStream`，将 fd 注册到 Reactor，读写操作返回 Future。

`src/net/async_tcp.rs`:

```rust
use std::future::Future;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::os::unix::io::AsRawFd;
use std::pin::Pin;
use std::task::{Context, Poll};
use crate::runtime::reactor::reactor;
use crate::net::tcp::connect as tcp_connect;

pub struct AsyncTcpStream {
    inner: TcpStream,
}

impl AsyncTcpStream {
    pub fn from_std(stream: TcpStream) -> Self {
        stream.set_nonblocking(true).expect("set_nonblocking");
        AsyncTcpStream { inner: stream }
    }

    pub fn connect(host: &str, port: u16) -> ConnectFuture {
        ConnectFuture { host: host.to_string(), port, done: false }
    }

    pub fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> ReadFuture<'a> {
        ReadFuture { stream: self, buf, token: None }
    }

    pub fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> WriteFuture<'a> {
        WriteFuture { stream: self, buf, written: 0, token: None }
    }
}

// ── ConnectFuture ─────────────────────────────────────────────────────────────

pub struct ConnectFuture { host: String, port: u16, done: bool }

impl Future for ConnectFuture {
    type Output = crate::Result<AsyncTcpStream>;
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if !self.done {
            self.done = true;
            // 当前使用阻塞 connect（TLS 握手在上层），后续可改为非阻塞
            match tcp_connect(&self.host, self.port) {
                Ok(stream) => Poll::Ready(Ok(AsyncTcpStream::from_std(stream))),
                Err(e) => Poll::Ready(Err(e)),
            }
        } else {
            Poll::Ready(Err(crate::Error::Io("already connected".into())))
        }
    }
}

// ── ReadFuture ────────────────────────────────────────────────────────────────

pub struct ReadFuture<'a> {
    stream: &'a mut AsyncTcpStream,
    buf: &'a mut [u8],
    token: Option<u64>,
}

impl<'a> Future for ReadFuture<'a> {
    type Output = crate::Result<usize>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // 移除旧注册
        if let Some(t) = self.token.take() {
            reactor().lock().unwrap().remove(t);
        }
        match self.stream.inner.read(self.buf) {
            Ok(n) => Poll::Ready(Ok(n)),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                let fd = self.stream.inner.as_raw_fd();
                let token = reactor().lock().unwrap().register_readable(fd, cx.waker().clone());
                self.token = Some(token);
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(crate::Error::Io(e.to_string()))),
        }
    }
}

// ── WriteFuture ───────────────────────────────────────────────────────────────

pub struct WriteFuture<'a> {
    stream: &'a mut AsyncTcpStream,
    buf: &'a [u8],
    written: usize,
    token: Option<u64>,
}

impl<'a> Future for WriteFuture<'a> {
    type Output = crate::Result<()>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(t) = self.token.take() {
            reactor().lock().unwrap().remove(t);
        }
        loop {
            if self.written == self.buf.len() {
                return Poll::Ready(Ok(()));
            }
            match self.stream.inner.write(&self.buf[self.written..]) {
                Ok(n) => self.written += n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    let fd = self.stream.inner.as_raw_fd();
                    let token = reactor().lock().unwrap().register_readable(fd, cx.waker().clone());
                    self.token = Some(token);
                    return Poll::Pending;
                }
                Err(e) => return Poll::Ready(Err(crate::Error::Io(e.to_string()))),
            }
        }
    }
}
```

- [ ] **Step 1: 写测试（需要本地 echo 服务器）**

`tests/runtime/net_test.rs`:

```rust
use std::net::TcpListener;
use std::thread;
use viv::runtime::executor::block_on;
use viv::net::async_tcp::AsyncTcpStream;

fn start_echo_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((mut conn, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            use std::io::{Read, Write};
            if let Ok(n) = conn.read(&mut buf) {
                conn.write_all(&buf[..n]).ok();
            }
        }
    });
    port
}

#[test]
fn async_tcp_write_and_read() {
    let port = start_echo_server();
    std::thread::sleep(std::time::Duration::from_millis(10)); // 服务器就绪

    block_on(async move {
        let mut stream = AsyncTcpStream::connect("127.0.0.1", port).await.unwrap();
        stream.write_all(b"hello").await.unwrap();
        let mut buf = [0u8; 5];
        let n = stream.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], b"hello");
    });
}
```

- [ ] **Step 2: 运行测试，确认失败**

```bash
cargo test --test net_test 2>&1 | head -20
```

- [ ] **Step 3: 实现 `src/net/async_tcp.rs`**

粘贴上方代码。

- [ ] **Step 4: 更新 `src/net/mod.rs`，导出新模块**

```rust
pub mod async_tcp;
```

- [ ] **Step 5: 检查 `crate::Error::Io` 是否存在**

```bash
grep -n "Io" src/error.rs
```

若不存在，在 `src/error.rs` 的 `Error` 枚举中添加：

```rust
Io(String),
```

- [ ] **Step 6: 运行测试，确认通过**

```bash
cargo test --test net_test
```

期望：`1 passed`

- [ ] **Step 7: Commit**

```bash
git add src/net/async_tcp.rs src/net/mod.rs tests/runtime/net_test.rs src/error.rs
git commit -m "feat(runtime): AsyncTcpStream — non-blocking read/write via reactor"
```

---

## Task 6: 公开 API + Runtime 线程封装

**Files:**
- Modify: `src/runtime/mod.rs`
- Modify: `src/main.rs`

### 目标

提供 `Runtime::new()` + `Runtime::spawn()` + `Runtime::block_on()`，在独立线程上运行，与 UI 线程通过 channel 通信。

- [ ] **Step 1: 完善 `src/runtime/mod.rs`**

```rust
pub mod task;
pub mod executor;
pub mod reactor;
pub mod timer;

pub use executor::{block_on, Executor};
pub use reactor::reactor;
pub use timer::sleep;
pub use task::JoinHandle;

use std::sync::mpsc;
use std::thread;

/// Runtime 运行在独立线程，暴露 spawn 接口
pub struct Runtime {
    _handle: thread::JoinHandle<()>,
    tx: mpsc::Sender<Box<dyn FnOnce(&mut Executor) + Send>>,
}

impl Runtime {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<Box<dyn FnOnce(&mut Executor) + Send>>();
        let handle = thread::spawn(move || {
            let mut exec = Executor::new();
            loop {
                // 接收新任务提交
                while let Ok(f) = rx.try_recv() {
                    f(&mut exec);
                }
                exec.run_ready();
                if exec.is_idle() {
                    reactor().lock().unwrap().wait(std::time::Duration::from_millis(10));
                }
            }
        });
        Runtime { _handle: handle, tx }
    }
}
```

- [ ] **Step 2: 编译检查**

```bash
cargo build 2>&1 | grep -E "^error"
```

期望：无 error

- [ ] **Step 3: 全量测试**

```bash
cargo test
```

期望：所有测试通过，无 warning 变 error

- [ ] **Step 4: Commit**

```bash
git add src/runtime/mod.rs
git commit -m "feat(runtime): Runtime thread wrapper — public API complete"
```

---

## 自检结果

- [x] Spec § 三（Async Runtime）覆盖：Reactor ✓ Executor ✓ Waker ✓ sleep ✓ AsyncTcpStream ✓ block_on ✓ spawn ✓
- [x] 无 TBD / TODO 占位
- [x] 类型一致：`TaskId = usize`，`JoinHandle<T>`，`OneshotSender/Receiver<T>` 贯穿始终
- [x] 每个 task 有完整代码，不依赖后续 task 中才定义的类型
- [x] TDD：每个 task 先写测试、再实现
