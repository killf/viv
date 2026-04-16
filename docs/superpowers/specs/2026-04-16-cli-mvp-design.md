# viv CLI MVP 设计文档

## 概述

viv 是一个自我进化的 AI 编程 Agent。本文档定义 CLI 的 MVP 设计：一个交互式 REPL，通过 Claude API 进行流式对话。

**技术选型：**
- Rust nightly edition 2024，单 crate
- **零外部依赖**，仅使用 std 和 libc 系统调用
- 终端控制、事件循环、HTTP 客户端、JSON 解析全部从零实现
- TDD 开发：每个模块先写测试再写实现

## 架构

### 模块结构

```
src/
├── main.rs          # 入口，初始化并启动 REPL
├── terminal/        # 终端控制层
│   ├── mod.rs
│   ├── raw_mode.rs  # termios 操作、RAII 恢复
│   ├── input.rs     # 原始字节 → 按键事件解析
│   ├── output.rs    # ANSI 转义序列输出
│   └── screen.rs    # 双缓冲区 diff 渲染
├── event.rs         # 事件类型定义与事件循环（基于 epoll）
├── repl.rs          # REPL 主循环、输入编辑、会话管理
├── net/             # 网络层
│   ├── mod.rs
│   ├── tcp.rs       # TCP 连接（std::net）
│   ├── tls.rs       # TLS 握手与加密传输（自实现或系统 OpenSSL FFI）
│   ├── http.rs      # HTTP/1.1 请求构造与响应解析
│   └── sse.rs       # Server-Sent Events 流解析
└── json.rs          # JSON 序列化/反序列化
```

### 1. 终端控制层 (`terminal/`)

从零实现终端控制，不依赖任何第三方库。

**raw_mode.rs**
- 通过 `libc::termios`（unsafe FFI）直接操作终端属性（关闭 canonical mode、echo）
- RAII 模式：`RawMode` 结构体在 `Drop` 时自动恢复终端状态
- 确保 panic 和异常退出时终端也能恢复

**input.rs**
- 从 stdin 读取原始字节（非阻塞模式，配合 epoll）
- 解析为 `KeyEvent` 枚举：普通字符（含 UTF-8 多字节）、方向键、Ctrl 组合键、Shift+Enter、粘贴序列等

**output.rs**
- ANSI 转义序列封装：光标移动、清屏/清行、前景/背景色、粗体/下划线等样式
- 写入 stdout 的缓冲写入器

**screen.rs**
- 双缓冲区：前缓冲区（当前屏幕状态）和后缓冲区（目标状态）
- diff 渲染：比较两个缓冲区，只输出变化的单元格
- 减少闪烁，提升渲染性能

### 2. 事件循环 (`event.rs`)

不依赖 Tokio，基于 Linux epoll 系统调用实现事件多路复用：

- `epoll_create` / `epoll_ctl` / `epoll_wait`（通过 libc FFI）
- 同时监听多个 fd：
  - stdin fd — 终端输入事件
  - TCP socket fd — LLM 流式响应数据
  - signalfd — SIGINT（中断生成）、SIGWINCH（终端大小变化）
- 单线程事件循环，非阻塞 IO

### 3. 网络层 (`net/`)

从零实现 HTTPS 客户端，不依赖 reqwest。

**tcp.rs**
- 基于 `std::net::TcpStream` 建立 TCP 连接

**tls.rs**
- 通过 FFI 调用系统 OpenSSL（`libssl`/`libcrypto`）实现 TLS
- 仅实现 Claude API 所需的最小 TLS 客户端功能

**http.rs**
- 手写 HTTP/1.1 请求构造（POST + headers + body）
- 解析 HTTP 响应（status line、headers、chunked transfer encoding）

**sse.rs**
- 解析 `text/event-stream` 格式
- 提取 `event:` 和 `data:` 字段，识别 `content_block_delta` 事件

### 4. JSON (`json.rs`)

手写 JSON 序列化/反序列化：
- 序列化：将 Rust 结构体构造为 JSON 字符串（用于 API 请求 body）
- 反序列化：解析 JSON 字符串为结构体（用于 API 响应解析）
- 仅需支持 Claude Messages API 涉及的类型：object、array、string、number、bool、null

### 5. REPL 交互 (`repl.rs`)

- **输入编辑器**：光标左右移动、删除（Backspace/Delete）、Home/End 跳转、多行输入（Shift+Enter 换行，Enter 提交）
- **流式渲染**：收到 LLM 的 SSE chunk 后逐字追加到屏幕，用户可 Ctrl+C 中断生成
- **会话状态**：内存中维护 `Vec<Message>` 作为对话上下文
- **退出**：Ctrl+D 或输入 `/exit`

### 6. Claude API 集成

- 调用 `https://api.anthropic.com/v1/messages`
- 请求参数：model、messages、max_tokens、stream=true
- 通过自实现的 net 层发送 HTTPS POST 请求
- 解析 SSE 流，提取 `content_block_delta` 中的文本
- API Key 从环境变量 `ANTHROPIC_API_KEY` 读取

## 数据流

```
用户按键 → input.rs 解析 → KeyEvent
                                ↓
                        event loop (epoll)
                                ↓
                          repl.rs 处理
                                ↓ (Enter 提交)
              json.rs 序列化 → http.rs 构造请求 → tls.rs 加密 → TCP 发送
                                                                    ↓
              TCP 接收 → tls.rs 解密 → http.rs 解析 → sse.rs 提取 chunk
                                                                    ↓
                        event loop (epoll) → repl.rs → screen.rs 渲染
```

## 依赖

```toml
[dependencies]
# 零依赖 — 仅使用 std 和 libc 系统调用
# TLS 通过 FFI 调用系统 libssl（运行时链接）
```

## 开发方式

采用 TDD：每个模块先写单元测试定义行为，再写实现使测试通过。

测试策略：
- `terminal/raw_mode.rs`：测试 termios 属性设置和 RAII 恢复
- `terminal/input.rs`：测试字节序列到 KeyEvent 的解析（普通字符、转义序列、UTF-8）
- `terminal/output.rs`：测试 ANSI 序列生成的正确性
- `terminal/screen.rs`：测试双缓冲区 diff 算法
- `event.rs`：测试 epoll 事件分发
- `net/http.rs`：测试 HTTP 请求构造和响应解析
- `net/sse.rs`：测试 SSE 流解析
- `json.rs`：测试 JSON 序列化/反序列化
- `repl.rs`：测试输入编辑操作和会话状态管理
