# viv CLI MVP 设计文档

## 概述

viv 是一个自我进化的 AI 编程 Agent。本文档定义 CLI 的 MVP 设计：一个交互式 REPL，通过 Claude API 进行流式对话。

**技术选型：**
- Rust nightly edition 2024，单 crate
- 终端控制完全从零实现（不依赖 Crossterm/Ratatui）
- Tokio 异步运行时 + reqwest HTTP 客户端
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
├── event.rs         # 事件类型定义与事件循环
├── repl.rs          # REPL 主循环、输入编辑、会话管理
└── api.rs           # Claude API 调用与 SSE 解析
```

### 1. 终端控制层 (`terminal/`)

从零实现终端控制，不依赖第三方 TUI 库。

**raw_mode.rs**
- 通过 `libc::termios` 直接操作终端属性（关闭 canonical mode、echo）
- RAII 模式：`RawMode` 结构体在 `Drop` 时自动恢复终端状态
- 确保 panic 和异常退出时终端也能恢复

**input.rs**
- 从 stdin 异步读取原始字节
- 解析为 `KeyEvent` 枚举：普通字符（含 UTF-8 多字节）、方向键、Ctrl 组合键、Shift+Enter、粘贴序列等

**output.rs**
- ANSI 转义序列封装：光标移动、清屏/清行、前景/背景色、粗体/下划线等样式
- 写入 stdout 的缓冲写入器

**screen.rs**
- 双缓冲区：前缓冲区（当前屏幕状态）和后缓冲区（目标状态）
- diff 渲染：比较两个缓冲区，只输出变化的单元格
- 减少闪烁，提升渲染性能

### 2. 事件循环 (`event.rs`)

- 基于 Tokio 异步运行时
- `tokio::select!` 多路复用监听：
  - 终端输入事件（`KeyEvent`）
  - LLM 流式响应块（`StreamChunk`）
  - 系统信号（`SIGINT` 中断生成、`SIGWINCH` 终端窗口大小变化）
- 模块间通过 `tokio::mpsc` 通道解耦

### 3. REPL 交互 (`repl.rs`)

- **输入编辑器**：光标左右移动、删除（Backspace/Delete）、Home/End 跳转、多行输入（Shift+Enter 换行，Enter 提交）
- **流式渲染**：收到 LLM 的 SSE chunk 后逐字追加到屏幕，用户可 Ctrl+C 中断生成
- **会话状态**：内存中维护 `Vec<Message>` 作为对话上下文
- **退出**：Ctrl+D 或输入 `/exit`

### 4. Claude API (`api.rs`)

- 直接用 `reqwest` 调用 `https://api.anthropic.com/v1/messages`
- 请求参数：model、messages、max_tokens、stream=true
- 自行解析 `text/event-stream` 响应，提取 `content_block_delta` 中的文本
- API Key 从环境变量 `ANTHROPIC_API_KEY` 读取

## 数据流

```
用户按键 → input.rs 解析 → KeyEvent → event loop
                                         ↓
                                    repl.rs 处理
                                         ↓ (Enter 提交)
                                    api.rs 发送请求
                                         ↓ (SSE stream)
                                    StreamChunk → event loop → repl.rs → screen.rs 渲染
```

## 依赖

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["stream"] }
libc = "0.2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

## 开发方式

采用 TDD：每个模块先写单元测试定义行为，再写实现使测试通过。

测试策略：
- `terminal/raw_mode.rs`：测试 termios 属性设置和 RAII 恢复
- `terminal/input.rs`：测试字节序列到 KeyEvent 的解析（普通字符、转义序列、UTF-8）
- `terminal/output.rs`：测试 ANSI 序列生成的正确性
- `terminal/screen.rs`：测试双缓冲区 diff 算法
- `event.rs`：测试事件分发
- `api.rs`：测试 SSE 解析逻辑（mock HTTP 响应）
- `repl.rs`：测试输入编辑操作和会话状态管理
