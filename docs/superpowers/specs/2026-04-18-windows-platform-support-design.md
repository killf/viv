# Windows 平台支持设计

## 概述

为 viv 添加完整的 Windows 平台支持，实现所有 Linux 功能在 Windows 上的对齐。采用平台模块分离架构，通过统一 trait 抽象屏蔽 OS 差异，上层代码零平台感知。

## 约束与决策

| 决策 | 选择 | 理由 |
|------|------|------|
| Windows 最低版本 | Windows 10 1607+ | 支持 VT 转义序列，复用现有 ANSI 渲染 |
| 事件循环 | IOCP (I/O Completion Ports) | 性能最好的 Windows 异步 I/O 机制 |
| 架构策略 | 平台模块分离 (方案 A) | 最清晰，为 AgentOS 长期目标做正确投资 |
| Shell 执行 | PowerShell | 现代 Windows 标配，功能与 bash 对等 |
| TLS Drop 写入 | std::io::Write | 替代内联汇编 syscall，跨平台 |
| 抽象方式 | 类型别名 (非 trait object) | 零开销，符合零依赖理念 |

## 目录结构

```
src/core/platform/
├── mod.rs              # cfg 选择 + 重导出统一接口
├── types.rs            # 跨平台类型别名 (RawHandle, etc.)
├── unix/
│   ├── mod.rs
│   ├── reactor.rs      # epoll Reactor 实现
│   ├── timer.rs        # timerfd 实现
│   ├── notifier.rs     # pipe 唤醒机制
│   ├── terminal.rs     # tcgetattr/tcsetattr, ioctl, SIGWINCH
│   └── process.rs      # sh -c 执行
└── windows/
    ├── mod.rs
    ├── ffi.rs           # 所有 Windows API FFI 声明
    ├── reactor.rs       # IOCP Reactor 实现
    ├── timer.rs         # CreateWaitableTimer 实现
    ├── notifier.rs      # Event object 唤醒
    ├── terminal.rs      # GetConsoleMode, GetConsoleScreenBufferInfo, ReadConsoleInput
    └── process.rs       # powershell 执行
```

## 核心 Trait 抽象

### RawHandle 跨平台类型

```rust
// platform/types.rs
#[cfg(unix)]
pub type RawHandle = std::os::unix::io::RawFd;  // i32

#[cfg(windows)]
pub type RawHandle = std::os::windows::io::RawHandle;  // *mut c_void
```

### Reactor trait

统一 epoll (reactor 模型) 和 IOCP (proactor 模型) 的接口。抽象层面向完成事件，在 Unix 上用 epoll + 立即 I/O 模拟 proactor 语义。

```rust
pub trait Reactor: Send + Sync {
    /// 注册一个可读兴趣，返回 token
    fn register_read(&self, handle: RawHandle, waker: Waker) -> crate::Result<u64>;

    /// 注册一个可写兴趣，返回 token
    fn register_write(&self, handle: RawHandle, waker: Waker) -> crate::Result<u64>;

    /// 取消注册
    fn deregister(&self, token: u64) -> crate::Result<()>;

    /// 等待事件，唤醒对应 waker，返回触发的事件数
    fn poll(&self, timeout: Duration) -> crate::Result<usize>;
}
```

平台导出为类型别名：

```rust
// platform/mod.rs
#[cfg(unix)]
pub type PlatformReactor = unix::EpollReactor;
#[cfg(windows)]
pub type PlatformReactor = windows::IocpReactor;
```

### EventNotifier trait

替代 channel.rs 中的 pipe 唤醒机制，用于从同步代码唤醒 async reactor。

```rust
pub trait EventNotifier: Send + Sync {
    /// 发送唤醒信号
    fn notify(&self) -> crate::Result<()>;

    /// 获取可注册到 Reactor 的 handle
    fn handle(&self) -> RawHandle;

    /// 消费通知（读走数据/重置事件）
    fn drain(&self) -> crate::Result<()>;
}
```

Unix 实现：`pipe()` + `write` 1 byte / `read` drain。
Windows 实现：`CreateEventW` + `SetEvent` / `ResetEvent`。

### PlatformTimer trait

```rust
pub trait PlatformTimer: Send {
    /// 创建定时器，返回可注册到 Reactor 的实例
    fn create(duration: Duration) -> crate::Result<Self> where Self: Sized;

    /// 获取 handle 用于 Reactor 注册
    fn handle(&self) -> RawHandle;

    /// 检查是否已到期（并消费通知）
    fn is_expired(&self) -> crate::Result<bool>;
}
```

Unix 实现：`timerfd_create` + `timerfd_settime`。
Windows 实现：`CreateWaitableTimerW` + `SetWaitableTimer`。

### PlatformTerminal trait

```rust
pub trait PlatformTerminal: Send {
    fn enable_raw_mode(&mut self) -> crate::Result<()>;
    fn disable_raw_mode(&mut self) -> crate::Result<()>;
    fn size(&self) -> crate::Result<(u16, u16)>;
    /// 获取终端输入的 handle（用于注册到 Reactor）
    fn input_handle(&self) -> RawHandle;
    /// 非阻塞读取输入字节
    fn read_input(&self, buf: &mut [u8]) -> crate::Result<usize>;
}
```

Unix 实现：`tcgetattr`/`tcsetattr` + `ioctl(TIOCGWINSZ)` + `/dev/tty`。
Windows 实现：`Get/SetConsoleMode`（启用 `ENABLE_VIRTUAL_TERMINAL_PROCESSING`）+ `GetConsoleScreenBufferInfo` + `ReadConsoleInput`。

### ResizeListener trait

```rust
pub trait ResizeListener: Send {
    /// 获取可注册到 Reactor 的 handle
    fn handle(&self) -> RawHandle;
    /// 消费 resize 事件，返回是否有 resize
    fn poll_resize(&self) -> crate::Result<bool>;
}
```

Unix 实现：`SIGWINCH` + self-pipe + `sigaction`。
Windows 实现：从 `ReadConsoleInput` 中过滤 `WINDOW_BUFFER_SIZE_EVENT`。

### shell_command 函数

```rust
// platform/unix/process.rs
pub fn shell_command(cmd: &str) -> std::process::Command {
    let mut c = std::process::Command::new("sh");
    c.arg("-c").arg(cmd);
    c
}

// platform/windows/process.rs
pub fn shell_command(cmd: &str) -> std::process::Command {
    let mut c = std::process::Command::new("powershell");
    c.args(&["-NoProfile", "-Command", cmd]);
    c
}
```

## 上层代码迁移

### runtime/reactor.rs

```rust
// 之前
use crate::core::event::Epoll;
pub struct Reactor { epoll: Epoll, ... }

// 之后
use crate::core::platform;
pub struct Reactor { inner: platform::PlatformReactor, ... }
```

waker 管理逻辑保持不变。

### runtime/timer.rs

```rust
// 之前：直接调 timerfd_create FFI
pub struct Sleep { fd: Option<RawFd>, ... }

// 之后
pub struct Sleep { timer: Option<platform::PlatformTimer>, ... }
```

Future::poll 逻辑不变，创建和检查 timer 改为调 trait 方法。

### runtime/channel.rs

```rust
// 之前
pub struct NotifySender<T> { tx: mpsc::Sender<T>, pipe: Arc<PipeWrite> }

// 之后
pub struct NotifySender<T> { tx: mpsc::Sender<T>, notifier: Arc<platform::PlatformNotifier> }
```

### terminal/backend.rs

新增 `CrossBackend`，统一替代 `LinuxBackend`：

```rust
pub struct CrossBackend {
    terminal: platform::PlatformTerminal,
    stdout: std::io::Stdout,  // ANSI 输出（Win10+ VT 模式通用）
}

impl Backend for CrossBackend { ... }
```

`LinuxBackend` 和 `TestBackend` 保留用于测试/兼容。`CrossBackend` 成为默认后端。

### bus/terminal.rs

```rust
// 之前
backend: LinuxBackend,

// 之后
backend: CrossBackend,
```

### terminal/events.rs

```rust
// 之前：直接用 epoll + open("/dev/tty") + SIGWINCH pipe
// 之后
pub struct EventLoop {
    reactor: platform::PlatformReactor,
    terminal: platform::PlatformTerminal,
    resize: platform::PlatformResizeListener,
}
```

### net/tls/mod.rs Drop

```rust
// 之前：内联汇编 x86_64 syscall
// 之后
impl Drop for TlsStream {
    fn drop(&mut self) {
        let _ = self.inner.write_all(&close_notify_record);
        let _ = self.inner.flush();
    }
}
```

### tools/bash.rs

```rust
// 之前
let mut child = Command::new("sh").arg("-c").arg(&command)...

// 之后
let mut child = platform::shell_command(&command)...
```

### mcp/transport/stdio.rs

当前用 `RawFd` + `fcntl` 设置非阻塞模式。迁移后使用 `platform::RawHandle` 和平台 notifier 机制集成到 reactor。

## 错误处理

统一使用 `std::io::Error::last_os_error()`，它在 Unix 上读 errno，Windows 上读 `GetLastError()`，天然跨平台。Error 枚举不需要平台门控变体：

```rust
pub enum Error {
    Io(std::io::Error),  // 涵盖所有平台 OS 错误
    // 其余现有变体不变...
}
```

每个 platform 模块提供统一的辅助函数：

```rust
pub(crate) fn last_os_error() -> crate::Error {
    crate::Error::Io(std::io::Error::last_os_error())
}
```

## Windows FFI 声明

集中在 `platform/windows/ffi.rs`，通过 `#[link(name = "kernel32")]` 链接：

```rust
#[link(name = "kernel32")]
unsafe extern "system" {
    // 终端
    fn GetConsoleMode(handle: RawHandle, mode: *mut u32) -> i32;
    fn SetConsoleMode(handle: RawHandle, mode: u32) -> i32;
    fn GetConsoleScreenBufferInfo(handle: RawHandle, info: *mut ConsoleScreenBufferInfo) -> i32;
    fn GetStdHandle(std_handle: u32) -> RawHandle;
    fn ReadConsoleInputW(handle: RawHandle, buf: *mut InputRecord, len: u32, read: *mut u32) -> i32;

    // IOCP
    fn CreateIoCompletionPort(file: RawHandle, existing: RawHandle, key: usize, threads: u32) -> RawHandle;
    fn GetQueuedCompletionStatus(port: RawHandle, bytes: *mut u32, key: *mut usize, overlapped: *mut *mut Overlapped, timeout: u32) -> i32;
    fn PostQueuedCompletionStatus(port: RawHandle, bytes: u32, key: usize, overlapped: *mut Overlapped) -> i32;

    // 定时器
    fn CreateWaitableTimerW(attrs: *mut u8, manual: i32, name: *const u16) -> RawHandle;
    fn SetWaitableTimer(timer: RawHandle, due: *const i64, period: i32, completion: usize, arg: usize, resume: i32) -> i32;

    // 事件对象
    fn CreateEventW(attrs: *mut u8, manual: i32, initial: i32, name: *const u16) -> RawHandle;
    fn SetEvent(event: RawHandle) -> i32;
    fn ResetEvent(event: RawHandle) -> i32;

    // 通用
    fn CloseHandle(handle: RawHandle) -> i32;
    fn GetLastError() -> u32;
}
```

## 测试策略

测试目录结构（镜像源码）：

```
tests/platform/
├── mod.rs
├── reactor_test.rs     # Reactor trait 行为测试
├── timer_test.rs       # PlatformTimer 行为测试
├── notifier_test.rs    # EventNotifier 行为测试
├── terminal_test.rs    # PlatformTerminal 行为测试
└── process_test.rs     # shell_command 测试
```

### 测试分层

- **平台无关行为测试**：验证 trait 契约（注册 handle → poll → waker 唤醒），在任何 OS 上运行
- **集成测试**：`--features full_test` 测完整 REPL 流程
- **CI 矩阵**：GitHub Actions 同时跑 `ubuntu-latest` 和 `windows-latest`

### 关键测试用例

| 模块 | 测试内容 |
|------|---------|
| Reactor | 注册读/写 → 触发事件 → waker 被唤醒 |
| Timer | 创建 100ms timer → poll → 到期回调 |
| Notifier | notify() → reactor poll → drain() 消费 |
| Terminal | enable_raw_mode → size() 有效 → disable_raw_mode |
| Process | shell_command("echo hello") → 输出 "hello" |

## 条件编译

不新增 Cargo feature flag。平台差异完全通过 `#[cfg(unix)]` / `#[cfg(windows)]` 处理。CI 中用 `--target x86_64-pc-windows-msvc` 交叉编译验证。

## 已跨平台的模块（无需修改）

以下模块为纯 Rust 实现，无平台依赖：

- 加密模块：TLS 1.3、AES-GCM、SHA256、X25519
- JSON / JSONRPC 解析
- LLM 客户端逻辑
- TUI 渲染层（Widget、Screen、布局）
- Agent / Memory / Permission 系统
