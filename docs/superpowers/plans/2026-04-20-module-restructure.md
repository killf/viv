# Module Restructure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 重新组织模块结构，消除职责错位，让目录层次更清晰地反映系统架构。

**Architecture:** 五处主要变动：① `bus/` 模块消除——`TerminalUI` 移入 `tui/terminal.rs`，消息协议和通道移入 `agent/`；② `llm.rs` 升级为独立子目录 `llm/`；③ `core/` 内密码学原语提升为 `core/crypto/`；④ `jsonrpc.rs` 下沉到 `lsp/`；⑤ `log/` 移入 `core/log/`（与 Agent 无关的基础层统一归 core）。所有变动只涉及文件移动和 `use` 路径更新，不改任何逻辑。

**Tech Stack:** Rust 2024 edition，`cargo test` 验证，`cargo clippy` 检查

---

## 目标目录结构

```
src/
├── main.rs
├── lib.rs
├── error.rs
├── config.rs
├── agent/
│   ├── mod.rs
│   ├── agent.rs
│   ├── channel.rs      ← 从 bus/channel.rs 移入（AgentHandle, AgentEndpoint, agent_channel）
│   ├── evolution.rs
│   ├── message.rs
│   ├── prompt.rs
│   └── protocol.rs     ← 从 bus/mod.rs 移入（AgentEvent, AgentMessage）
│                          （bus/ 目录整体消除）
├── core/
│   ├── mod.rs
│   ├── crypto/         ← 新增：密码学原语独立子目录
│   │   ├── mod.rs
│   │   ├── bigint.rs   ← 从 core/bigint.rs 移入
│   │   ├── asn1.rs     ← 从 core/asn1.rs 移入
│   │   ├── aes_gcm.rs  ← 从 core/net/tls/crypto/aes_gcm.rs 移入
│   │   ├── sha256.rs   ← 从 core/net/tls/crypto/sha256.rs 移入
│   │   └── x25519.rs   ← 从 core/net/tls/crypto/x25519.rs 移入
│   ├── json.rs         ← 保留
│   ├── event.rs        ← 保留
│   ├── sync.rs         ← 保留
│   ├── net/
│   │   ├── mod.rs
│   │   ├── tcp.rs
│   │   ├── async_tcp.rs
│   │   ├── http.rs
│   │   ├── sse.rs
│   │   ├── ws.rs
│   │   └── tls/        ← crypto/ 子目录移走后，其余文件保留
│   │       ├── mod.rs
│   │       ├── codec.rs
│   │       ├── ecdsa.rs
│   │       ├── handshake.rs
│   │       ├── key_schedule.rs
│   │       ├── p256.rs
│   │       ├── record.rs
│   │       ├── rsa.rs
│   │       └── x509.rs
│   ├── platform/       ← 保持不变
│   ├── runtime/        ← 保持不变
│   ├── terminal/       ← 保持不变
│   └── log/            ← 从根级 log/ 移入（与 Agent 无关的基础层）
│       ├── mod.rs
│       └── macros.rs
├── llm/                ← 从 llm.rs 升级为子目录
│   └── mod.rs          ← 原 llm.rs 全部内容
├── lsp/
│   ├── mod.rs
│   ├── jsonrpc.rs      ← 从 core/jsonrpc.rs 移入
│   ├── client.rs
│   ├── config.rs
│   ├── tools.rs
│   └── types.rs
├── log/
├── mcp/
├── memory/
├── permissions/
├── skill/
├── tools/
└── tui/
    ├── mod.rs
    ├── terminal.rs     ← 从 bus/terminal.rs 移入（TerminalUI）
    ├── block.rs
    ├── code_block.rs
    ├── content.rs
    ├── conversation.rs
    ├── focus.rs
    ├── header.rs
    ├── input.rs
    ├── lang_profiles.rs
    ├── layout.rs
    ├── markdown.rs
    ├── message_style.rs
    ├── paragraph.rs
    ├── permission.rs
    ├── qrcode/
    ├── renderer.rs
    ├── spinner.rs
    ├── status.rs
    ├── syntax.rs
    ├── tool_call.rs
    ├── welcome.rs
    └── widget.rs
```

---

## 执行顺序说明

四个任务相互独立，可按任意顺序执行，也可并行。但每个任务内部必须顺序执行。执行完每个任务后运行 `cargo build` 验证无编译错误，再提交。

---

## Task 1：消除 `bus/`，重组 `agent/` 和 `tui/`

移走 `terminal.rs` 后 `bus/` 只剩 59 行，且内容全是 Agent 通信协议。整体并入 `agent/`，`TerminalUI` 移入 `tui/`。

**Files:**
- Move: `src/bus/terminal.rs` → `src/tui/terminal.rs`
- Move: `src/bus/mod.rs` 内容 → `src/agent/protocol.rs`（AgentEvent, AgentMessage）
- Move: `src/bus/channel.rs` → `src/agent/channel.rs`（AgentHandle, AgentEndpoint, agent_channel）
- Delete: `src/bus/` 目录
- Modify: `src/lib.rs` — 删除 `pub mod bus;`
- Modify: `src/agent/mod.rs` — 添加 `pub mod channel; pub mod protocol;`
- Modify: `src/tui/mod.rs` — 添加 `pub mod terminal;`
- Modify: `src/main.rs` — 更新所有 use 路径
- Modify: 所有引用 `viv::bus::` 或 `crate::bus::` 的文件

- [ ] **Step 1: 确认所有引用点**

```bash
grep -r "crate::bus\|viv::bus\|use.*bus::" src/ --include="*.rs"
```

记录每个文件和引用的符号，下面逐一修改。

- [ ] **Step 2: 移动 TerminalUI**

```bash
mv src/bus/terminal.rs src/tui/terminal.rs
```

- [ ] **Step 3: 创建 `src/agent/protocol.rs`**

将 `src/bus/mod.rs` 中 `AgentEvent` 和 `AgentMessage` 的定义复制过来（去掉 `pub mod terminal;` 和 `pub mod channel;`）：

```rust
/// UI 线程 → Agent 线程
#[derive(Debug)]
pub enum AgentEvent {
    Input(String),
    PermissionResponse(bool),
    Interrupt,
    Quit,
}

/// Agent 线程 → UI 线程
#[derive(Debug)]
pub enum AgentMessage {
    Ready { model: String },
    Thinking,
    TextChunk(String),
    ToolStart { name: String, input: String },
    ToolEnd { name: String, output: String },
    ToolError { name: String, error: String },
    PermissionRequest { tool: String, input: String },
    Status(String),
    Tokens { input: u64, output: u64 },
    Done,
    Evolved,
    Error(String),
}
```

- [ ] **Step 4: 移动 `bus/channel.rs` → `agent/channel.rs`**

```bash
mv src/bus/channel.rs src/agent/channel.rs
```

更新 `src/agent/channel.rs` 内部的引用：

将：
```rust
use crate::bus::{AgentEvent, AgentMessage};
```
改为：
```rust
use crate::agent::protocol::{AgentEvent, AgentMessage};
```

- [ ] **Step 5: 更新 `src/agent/mod.rs`**

添加：
```rust
pub mod channel;
pub mod protocol;
```

- [ ] **Step 6: 更新 `src/tui/mod.rs`**

添加：
```rust
pub mod terminal;
```

- [ ] **Step 7: 更新 `src/lib.rs`**

删除：
```rust
pub mod bus;
```

- [ ] **Step 8: 更新所有引用 `bus::` 的文件**

将所有文件中的：
```rust
use crate::bus::{AgentEvent, AgentMessage};
use crate::bus::AgentEvent;
use crate::bus::AgentMessage;
```
改为：
```rust
use crate::agent::protocol::{AgentEvent, AgentMessage};
use crate::agent::protocol::AgentEvent;
use crate::agent::protocol::AgentMessage;
```

将：
```rust
use crate::bus::channel::{AgentHandle, AgentEndpoint, agent_channel};
use viv::bus::terminal::TerminalUI;
```
改为：
```rust
use crate::agent::channel::{AgentHandle, AgentEndpoint, agent_channel};
use viv::tui::terminal::TerminalUI;
```

- [ ] **Step 9: 删除 `bus/` 目录**

```bash
rm src/bus/mod.rs src/bus/channel.rs  # 已移走，确认为空
rmdir src/bus/
```

- [ ] **Step 10: 验证编译**

```bash
cargo build 2>&1 | head -40
```

期望：无 error

- [ ] **Step 11: 验证测试**

```bash
cargo test 2>&1 | tail -20
```

- [ ] **Step 12: Commit**

```bash
git add src/agent/protocol.rs src/agent/channel.rs src/agent/mod.rs \
        src/tui/terminal.rs src/tui/mod.rs \
        src/lib.rs src/main.rs
git commit -m "refactor: eliminate bus/, move protocol+channel to agent/, TerminalUI to tui/"
```

---

## Task 2：`llm.rs` → `llm/mod.rs`

**Files:**
- Create: `src/llm/mod.rs` — 原 `src/llm.rs` 全部内容
- Delete: `src/llm.rs`
- Modify: `src/lib.rs` — `pub mod llm;` 路径不变，无需修改（Rust 自动识别目录）

- [ ] **Step 1: 确认无需更改的引用**

```bash
grep -r "crate::llm\|viv::llm" src/ --include="*.rs" | head -20
```

Rust 对 `pub mod llm` 的解析：有 `src/llm.rs` 时用文件，有 `src/llm/mod.rs` 时用目录，两者等价，所有引用路径不变。

- [ ] **Step 2: 创建目录并移动**

```bash
mkdir -p src/llm
mv src/llm.rs src/llm/mod.rs
```

- [ ] **Step 3: 验证编译**

```bash
cargo build 2>&1 | head -30
```

期望：无 error

- [ ] **Step 4: Commit**

```bash
git add src/llm/ src/llm.rs
git commit -m "refactor: promote llm.rs to llm/ submodule directory"
```

---

## Task 3：`core/jsonrpc.rs` → `lsp/jsonrpc.rs`

**Files:**
- Move: `src/core/jsonrpc.rs` → `src/lsp/jsonrpc.rs`
- Modify: `src/core/mod.rs` — 删除 `pub mod jsonrpc;`
- Modify: `src/lsp/mod.rs` — 添加 `pub mod jsonrpc;`
- Modify: `src/lsp/client.rs` — 更新 use 路径（`crate::core::jsonrpc` → `crate::lsp::jsonrpc`）
- Modify: 其他所有引用 `core::jsonrpc` 的文件

- [ ] **Step 1: 确认所有引用点**

```bash
grep -r "core::jsonrpc\|jsonrpc::" src/ --include="*.rs"
```

记录所有出现位置，下面逐一修改。

- [ ] **Step 2: 移动文件**

```bash
mv src/core/jsonrpc.rs src/lsp/jsonrpc.rs
```

- [ ] **Step 3: 更新 `src/core/mod.rs`**

删除：
```rust
pub mod jsonrpc;
```

- [ ] **Step 4: 更新 `src/lsp/mod.rs`**

添加：
```rust
pub mod jsonrpc;
```

- [ ] **Step 5: 批量更新引用路径**

将所有文件中的：
```rust
use crate::core::jsonrpc::
```
改为：
```rust
use crate::lsp::jsonrpc::
```

如果 lsp 内部文件引用可简化为：
```rust
use super::jsonrpc::
// 或
use crate::lsp::jsonrpc::
```

- [ ] **Step 6: 验证编译**

```bash
cargo build 2>&1 | head -30
```

- [ ] **Step 7: Commit**

```bash
git add src/core/mod.rs src/core/jsonrpc.rs src/lsp/jsonrpc.rs src/lsp/mod.rs
git commit -m "refactor: move jsonrpc from core/ to lsp/ (only consumer)"
```

---

## Task 4：密码学原语升级为 `core/crypto/`

这是最大的重构任务。将散落在 `core/` 根和 `core/net/tls/crypto/` 的密码学原语统一到 `core/crypto/`。

**Files:**
- Create: `src/core/crypto/mod.rs`
- Move: `src/core/bigint.rs` → `src/core/crypto/bigint.rs`
- Move: `src/core/asn1.rs` → `src/core/crypto/asn1.rs`
- Move: `src/core/net/tls/crypto/aes_gcm.rs` → `src/core/crypto/aes_gcm.rs`
- Move: `src/core/net/tls/crypto/sha256.rs` → `src/core/crypto/sha256.rs`
- Move: `src/core/net/tls/crypto/x25519.rs` → `src/core/crypto/x25519.rs`
- Delete: `src/core/net/tls/crypto/` 目录（移空后删除）
- Modify: `src/core/mod.rs` — 用 `pub mod crypto;` 替换原来的 `pub mod bigint; pub mod asn1;`
- Modify: `src/core/net/tls/mod.rs` — 删除对 `crypto` 子模块的声明，改为引用 `crate::core::crypto`
- Modify: 所有引用旧路径的文件

- [ ] **Step 1: 确认所有引用点**

```bash
grep -r "core::bigint\|core::asn1\|tls::crypto\|net::tls::crypto" src/ --include="*.rs"
```

记录全部引用，下面逐一修改。

- [ ] **Step 2: 创建 `src/core/crypto/` 目录**

```bash
mkdir -p src/core/crypto
```

- [ ] **Step 3: 移动文件**

```bash
mv src/core/bigint.rs src/core/crypto/bigint.rs
mv src/core/asn1.rs src/core/crypto/asn1.rs
mv src/core/net/tls/crypto/aes_gcm.rs src/core/crypto/aes_gcm.rs
mv src/core/net/tls/crypto/sha256.rs src/core/crypto/sha256.rs
mv src/core/net/tls/crypto/x25519.rs src/core/crypto/x25519.rs
```

- [ ] **Step 4: 创建 `src/core/crypto/mod.rs`**

```rust
pub mod aes_gcm;
pub mod asn1;
pub mod bigint;
pub mod sha256;
pub mod x25519;
```

- [ ] **Step 5: 更新 `src/core/mod.rs`**

删除：
```rust
pub mod asn1;
pub mod bigint;
```

添加：
```rust
pub mod crypto;
```

- [ ] **Step 6: 删除空的 `src/core/net/tls/crypto/` 目录**

```bash
# 确认目录为空
ls src/core/net/tls/crypto/
# 删除旧的 mod.rs（内容已迁移）
rm src/core/net/tls/crypto/mod.rs
rmdir src/core/net/tls/crypto/
```

- [ ] **Step 7: 更新 `src/core/net/tls/mod.rs`**

删除：
```rust
mod crypto;
```
或原来的 `pub mod crypto;`

所有原来通过 `use super::crypto::` 或 `use crate::core::net::tls::crypto::` 的引用，改为 `use crate::core::crypto::`

- [ ] **Step 8: 批量更新内部文件路径**

在 `src/core/net/tls/` 下的各文件（codec.rs, ecdsa.rs, handshake.rs, key_schedule.rs, p256.rs, record.rs, rsa.rs, x509.rs）中：

将：
```rust
use super::crypto::aes_gcm::
use super::crypto::sha256::
use super::crypto::x25519::
```
改为：
```rust
use crate::core::crypto::aes_gcm::
use crate::core::crypto::sha256::
use crate::core::crypto::x25519::
```

在 `src/core/net/tls/x509.rs` 和 `src/core/net/tls/rsa.rs` 中原来引用 `core::asn1` / `core::bigint` 的路径：
```rust
use crate::core::asn1::
use crate::core::bigint::
```
改为：
```rust
use crate::core::crypto::asn1::
use crate::core::crypto::bigint::
```

在 `src/core/crypto/asn1.rs` 内部如果引用 `bigint`：
```rust
use super::bigint::
```

- [ ] **Step 9: 验证编译**

```bash
cargo build 2>&1 | head -50
```

期望：无 error。如有，根据编译器提示逐一修正路径。

- [ ] **Step 10: 运行测试**

```bash
cargo test 2>&1 | tail -30
```

- [ ] **Step 11: Commit**

```bash
git add src/core/crypto/ src/core/mod.rs src/core/bigint.rs src/core/asn1.rs \
        src/core/net/tls/crypto/ src/core/net/tls/mod.rs \
        src/core/net/tls/codec.rs src/core/net/tls/ecdsa.rs \
        src/core/net/tls/handshake.rs src/core/net/tls/key_schedule.rs \
        src/core/net/tls/p256.rs src/core/net/tls/record.rs \
        src/core/net/tls/rsa.rs src/core/net/tls/x509.rs
git commit -m "refactor: promote crypto primitives to core/crypto/ submodule"
```

---

## Task 5：`log/` → `core/log/`

`log/` 是纯基础设施（日志宏 + 日志写入），与 Agent 逻辑无关，归入 `core/` 与 `runtime/`、`platform/` 等基础层并列。

**Files:**
- Move: `src/log/` → `src/core/log/`
- Modify: `src/lib.rs` — 删除 `pub mod log;`
- Modify: `src/core/mod.rs` — 添加 `pub mod log;`
- Modify: 所有引用 `crate::log::` 或 `viv::log::` 的文件

- [ ] **Step 1: 确认所有引用点**

```bash
grep -r "crate::log\|viv::log\|use.*::log::" src/ --include="*.rs"
```

注意区分标准库的 `log` crate（本项目零依赖，应该没有）和自建的 `log` 模块。

- [ ] **Step 2: 移动目录**

```bash
mv src/log src/core/log
```

- [ ] **Step 3: 更新 `src/lib.rs`**

删除：
```rust
pub mod log;
```

- [ ] **Step 4: 更新 `src/core/mod.rs`**

添加：
```rust
pub mod log;
```

- [ ] **Step 5: 批量更新引用路径**

将所有文件中的：
```rust
use crate::log::
```
改为：
```rust
use crate::core::log::
```

如果有宏引用（`log_info!` 等），宏通过 `#[macro_export]` 导出到 crate 根，需确认宏是否需要调整路径。

```bash
grep -r "log_\|crate::log" src/ --include="*.rs" | head -30
```

- [ ] **Step 6: 验证编译**

```bash
cargo build 2>&1 | head -30
```

- [ ] **Step 7: Commit**

```bash
git add src/log/ src/core/log/ src/lib.rs src/core/mod.rs
git commit -m "refactor: move log/ into core/ (infrastructure belongs in core)"
```

---

## Task 6：新增 `core/encoding/`（Base64）和 `core/crypto/md5.rs`

这是唯一一个**新增实现**的任务（其他任务都是文件移动）。Base64 属于编码，归 `core/encoding/`；MD5 是哈希函数，归 `core/crypto/`，与 `sha256.rs` 并列。按 TDD 顺序实现。

**Files:**
- Create: `src/core/encoding/mod.rs`
- Create: `src/core/encoding/base64.rs`
- Create: `src/core/crypto/md5.rs`
- Modify: `src/core/mod.rs` — 添加 `pub mod encoding;`
- Modify: `src/core/crypto/mod.rs` — 添加 `pub mod md5;`
- Create: `tests/core/encoding/mod.rs`
- Create: `tests/core/encoding/base64_test.rs`
- Create: `tests/core/crypto/md5_test.rs`
- Modify: `tests/core/mod.rs` — 添加 `pub mod encoding;`
- Modify: `tests/core/crypto/mod.rs` — 添加 `mod md5_test;`

---

### Task 6a：Base64

- [ ] **Step 1: 创建测试文件 `tests/core/encoding/base64_test.rs`**

```rust
use viv::core::encoding::base64;

#[test]
fn encode_empty() {
    assert_eq!(base64::encode(b""), "");
}

#[test]
fn encode_one_byte() {
    assert_eq!(base64::encode(b"M"), "TQ==");
}

#[test]
fn encode_two_bytes() {
    assert_eq!(base64::encode(b"Ma"), "TWE=");
}

#[test]
fn encode_three_bytes() {
    assert_eq!(base64::encode(b"Man"), "TWFu");
}

#[test]
fn encode_longer() {
    assert_eq!(base64::encode(b"Hello, World!"), "SGVsbG8sIFdvcmxkIQ==");
}

#[test]
fn decode_empty() {
    assert_eq!(base64::decode("").unwrap(), b"");
}

#[test]
fn decode_one_byte() {
    assert_eq!(base64::decode("TQ==").unwrap(), b"M");
}

#[test]
fn decode_two_bytes() {
    assert_eq!(base64::decode("TWE=").unwrap(), b"Ma");
}

#[test]
fn decode_three_bytes() {
    assert_eq!(base64::decode("TWFu").unwrap(), b"Man");
}

#[test]
fn decode_longer() {
    assert_eq!(base64::decode("SGVsbG8sIFdvcmxkIQ==").unwrap(), b"Hello, World!");
}

#[test]
fn roundtrip() {
    let input = b"The quick brown fox jumps over the lazy dog";
    assert_eq!(base64::decode(&base64::encode(input)).unwrap(), input);
}

#[test]
fn decode_invalid_char() {
    assert!(base64::decode("TQ==!").is_err());
}
```

- [ ] **Step 2: 创建 `tests/core/encoding/mod.rs`**

```rust
mod base64_test;
```

- [ ] **Step 3: 更新 `tests/core/mod.rs`**

添加：
```rust
pub mod encoding;
```

- [ ] **Step 4: 运行测试确认失败**

```bash
cargo test core::encoding 2>&1 | tail -10
```

期望：编译失败，`viv::core::encoding` 不存在。

- [ ] **Step 5: 创建 `src/core/encoding/mod.rs`**

```rust
pub mod base64;
```

- [ ] **Step 6: 更新 `src/core/mod.rs`**

添加：
```rust
pub mod encoding;
```

- [ ] **Step 7: 实现 `src/core/encoding/base64.rs`**

```rust
use crate::error::Error;
use crate::Result;

const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn encode(input: &[u8]) -> String {
    let mut out = Vec::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((n >> 18) & 0x3f) as usize]);
        out.push(ALPHABET[((n >> 12) & 0x3f) as usize]);
        out.push(if chunk.len() > 1 { ALPHABET[((n >> 6) & 0x3f) as usize] } else { b'=' });
        out.push(if chunk.len() > 2 { ALPHABET[(n & 0x3f) as usize] } else { b'=' });
    }
    String::from_utf8(out).unwrap()
}

pub fn decode(input: &str) -> Result<Vec<u8>> {
    let input = input.as_bytes();
    if input.len() % 4 != 0 {
        return Err(Error::Invariant("base64: invalid length".into()));
    }
    let mut table = [0xffu8; 256];
    for (i, &c) in ALPHABET.iter().enumerate() {
        table[c as usize] = i as u8;
    }
    table[b'=' as usize] = 0;

    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    for chunk in input.chunks(4) {
        for &b in chunk.iter().take(2) {
            if b != b'=' && table[b as usize] == 0xff {
                return Err(Error::Invariant("base64: invalid character".into()));
            }
        }
        let n = ((table[chunk[0] as usize] as u32) << 18)
              | ((table[chunk[1] as usize] as u32) << 12)
              | ((table[chunk[2] as usize] as u32) << 6)
              |  (table[chunk[3] as usize] as u32);
        out.push((n >> 16) as u8);
        if chunk[2] != b'=' { out.push((n >> 8) as u8); }
        if chunk[3] != b'=' { out.push(n as u8); }
    }
    Ok(out)
}
```

- [ ] **Step 8: 运行测试确认通过**

```bash
cargo test core::encoding 2>&1 | tail -15
```

期望：所有 base64 测试 PASS。

- [ ] **Step 9: Commit**

```bash
git add src/core/encoding/ src/core/mod.rs \
        tests/core/encoding/ tests/core/mod.rs
git commit -m "feat: add core/encoding/base64 with encode/decode"
```

---

### Task 6b：MD5

- [ ] **Step 1: 创建测试文件 `tests/core/crypto/md5_test.rs`**

```rust
use viv::core::crypto::md5;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[test]
fn md5_empty() {
    assert_eq!(hex(&md5::md5(b"")), "d41d8cd98f00b204e9800998ecf8427e");
}

#[test]
fn md5_hello() {
    assert_eq!(hex(&md5::md5(b"hello")), "5d41402abc4b2a76b9719d911017c592");
}

#[test]
fn md5_abc() {
    assert_eq!(hex(&md5::md5(b"abc")), "900150983cd24fb0d6963f7d28e17f72");
}

#[test]
fn md5_long() {
    assert_eq!(
        hex(&md5::md5(b"The quick brown fox jumps over the lazy dog")),
        "9e107d9d372bb6826bd81d3542a419d6"
    );
}
```

- [ ] **Step 2: 更新 `tests/core/crypto/mod.rs`**

添加：
```rust
mod md5_test;
```

- [ ] **Step 3: 运行测试确认失败**

```bash
cargo test core::crypto::md5 2>&1 | tail -10
```

期望：编译失败，`md5` 模块不存在。

- [ ] **Step 4: 实现 `src/core/crypto/md5.rs`**

```rust
pub fn md5(input: &[u8]) -> [u8; 16] {
    // 预计算 T[i] = floor(abs(sin(i+1)) * 2^32)
    const T: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee,
        0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
        0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be,
        0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
        0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa,
        0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
        0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
        0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c,
        0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
        0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05,
        0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
        0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039,
        0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1,
        0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
    ];
    const S: [u32; 64] = [
        7,12,17,22, 7,12,17,22, 7,12,17,22, 7,12,17,22,
        5, 9,14,20, 5, 9,14,20, 5, 9,14,20, 5, 9,14,20,
        4,11,16,23, 4,11,16,23, 4,11,16,23, 4,11,16,23,
        6,10,15,21, 6,10,15,21, 6,10,15,21, 6,10,15,21,
    ];

    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut msg = input.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());

    let mut a0: u32 = 0x67452301;
    let mut b0: u32 = 0xefcdab89;
    let mut c0: u32 = 0x98badcfe;
    let mut d0: u32 = 0x10325476;

    for chunk in msg.chunks(64) {
        let mut m = [0u32; 16];
        for (i, w) in m.iter_mut().enumerate() {
            *w = u32::from_le_bytes(chunk[i*4..i*4+4].try_into().unwrap());
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0u32..64 {
            let (f, g) = match i {
                0..=15  => ((b & c) | (!b & d),          i),
                16..=31 => ((d & b) | (!d & c),          (5*i + 1) % 16),
                32..=47 => (b ^ c ^ d,                   (3*i + 5) % 16),
                _       => (c ^ (b | !d),                (7*i) % 16),
            };
            let temp = d;
            d = c;
            c = b;
            b = b.wrapping_add(
                (a.wrapping_add(f).wrapping_add(T[i as usize]).wrapping_add(m[g as usize]))
                    .rotate_left(S[i as usize])
            );
            a = temp;
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a0.to_le_bytes());
    out[4..8].copy_from_slice(&b0.to_le_bytes());
    out[8..12].copy_from_slice(&c0.to_le_bytes());
    out[12..16].copy_from_slice(&d0.to_le_bytes());
    out
}
```

- [ ] **Step 5: 更新 `src/core/crypto/mod.rs`**

添加：
```rust
pub mod md5;
```

- [ ] **Step 6: 运行测试确认通过**

```bash
cargo test core::crypto::md5 2>&1 | tail -10
```

期望：4 个测试全部 PASS。

- [ ] **Step 7: Commit**

```bash
git add src/core/crypto/md5.rs src/core/crypto/mod.rs \
        tests/core/crypto/md5_test.rs tests/core/crypto/mod.rs
git commit -m "feat: add core/crypto/md5 hash implementation"
```

---

## Task 7：`#[cfg]` 只允许出现在 `core/platform/`

**原则**：所有 `#[cfg(unix)]` / `#[cfg(windows)]` 分支必须封装在 `core/platform/` 内部，上层模块调用平台无关接口。

当前违规分布：
- `core/net/async_tcp.rs` — `AsRawFd` vs `AsRawSocket`（1 个函数）
- `core/terminal/size.rs` — ioctl 重复（platform 已有）
- `core/terminal/events.rs` — 直接调 `epoll_ctl` / `epoll_wait` FFI（reactor 接口不完整）
- `lsp/` + `mcp/transport/stdio.rs` — 子进程 + pipe（platform 完全无此抽象）

---

### Task 7a：`tcp_raw_handle` 归入 platform

**Files:**
- Modify: `src/core/platform/unix/mod.rs` — 添加 `pub fn tcp_raw_handle`
- Modify: `src/core/platform/windows/mod.rs` — 添加 `pub fn tcp_raw_handle`
- Modify: `src/core/platform/mod.rs` — re-export
- Modify: `src/core/net/async_tcp.rs` — 删除 cfg，调用 platform 函数

- [ ] **Step 1: 在 `src/core/platform/unix/mod.rs` 添加**

```rust
pub fn tcp_raw_handle(stream: &std::net::TcpStream) -> super::types::RawHandle {
    use std::os::unix::io::AsRawFd;
    stream.as_raw_fd()
}
```

- [ ] **Step 2: 在 `src/core/platform/windows/mod.rs` 添加**

```rust
pub fn tcp_raw_handle(stream: &std::net::TcpStream) -> super::types::RawHandle {
    use std::os::windows::io::AsRawSocket;
    stream.as_raw_socket() as super::types::RawHandle
}
```

- [ ] **Step 3: 在 `src/core/platform/mod.rs` 添加 re-export**

```rust
#[cfg(unix)]
pub use unix::tcp_raw_handle;
#[cfg(windows)]
pub use windows::tcp_raw_handle;
```

- [ ] **Step 4: 更新 `src/core/net/async_tcp.rs`**

删除：
```rust
#[cfg(unix)]
use std::os::unix::io::AsRawFd;
#[cfg(windows)]
use std::os::windows::io::AsRawSocket;
```

添加：
```rust
use crate::core::platform;
```

将方法：
```rust
pub fn raw_handle(&self) -> crate::core::platform::RawHandle {
    #[cfg(unix)]
    { self.inner.as_raw_fd() }
    #[cfg(windows)]
    { self.inner.as_raw_socket() as crate::core::platform::RawHandle }
}
```
改为：
```rust
pub fn raw_handle(&self) -> crate::core::platform::RawHandle {
    platform::tcp_raw_handle(&self.inner)
}
```

- [ ] **Step 5: 验证编译并提交**

```bash
cargo build 2>&1 | grep "^error" | head -10
git add src/core/platform/unix/mod.rs src/core/platform/windows/mod.rs \
        src/core/platform/mod.rs src/core/net/async_tcp.rs
git commit -m "refactor: move tcp_raw_handle cfg branch into platform/"
```

---

### Task 7b：`terminal_size` 去重——调用 platform 而非直接 ioctl

`platform/unix/terminal.rs` 已有 ioctl/TIOCGWINSZ，`terminal/size.rs` 重复实现了一遍。将独立的 `terminal_size()` 函数提升到 platform，`terminal/size.rs` 改为调用它。

**Files:**
- Modify: `src/core/platform/unix/terminal.rs` — 添加 `pub fn terminal_size`
- Modify: `src/core/platform/windows/terminal.rs` — 添加 stub
- Modify: `src/core/platform/mod.rs` — re-export `terminal_size`
- Modify: `src/core/terminal/size.rs` — 删除 `unix_impl`，改调 platform

- [ ] **Step 1: 在 `src/core/platform/unix/terminal.rs` 添加独立函数**

（ioctl 和 Winsize 结构体已在文件中定义，复用即可）

```rust
pub fn terminal_size() -> crate::Result<(u16, u16)> {
    #[repr(C)]
    struct Winsize { ws_row: u16, ws_col: u16, ws_xpixel: u16, ws_ypixel: u16 }
    const TIOCGWINSZ: u64 = 0x5413;
    let mut ws = Winsize { ws_row: 0, ws_col: 0, ws_xpixel: 0, ws_ypixel: 0 };
    let ret = unsafe { ioctl(1, TIOCGWINSZ, &mut ws) };
    if ret == 0 && ws.ws_col > 0 && ws.ws_row > 0 {
        Ok((ws.ws_row, ws.ws_col))
    } else {
        Ok((24, 80))
    }
}
```

- [ ] **Step 2: 在 `src/core/platform/windows/terminal.rs` 添加 stub**

```rust
pub fn terminal_size() -> crate::Result<(u16, u16)> {
    Ok((24, 80)) // TODO: use GetConsoleScreenBufferInfo
}
```

- [ ] **Step 3: 在 `src/core/platform/mod.rs` re-export**

```rust
#[cfg(unix)]
pub use unix::terminal::terminal_size;
#[cfg(windows)]
pub use windows::terminal::terminal_size;
```

Wait — `platform/mod.rs` re-exports are currently done through type aliases like `pub type PlatformTerminal = unix::UnixTerminal`. For functions use direct re-export:

```rust
#[cfg(unix)]
pub use unix::terminal_size;
#[cfg(windows)]
pub use windows::terminal_size;
```

（确认 `unix/mod.rs` 中 `pub use terminal::terminal_size;` 已导出）

- [ ] **Step 4: 更新 `src/core/terminal/size.rs`**

删除整个 `unix_impl` 模块和 `#[cfg(unix)] pub use unix_impl::terminal_size;`，改为：

```rust
use crate::core::platform;

/// Terminal dimensions in columns and rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TermSize {
    pub cols: u16,
    pub rows: u16,
}

pub fn terminal_size() -> crate::Result<TermSize> {
    let (rows, cols) = platform::terminal_size()?;
    Ok(TermSize { rows, cols })
}
```

- [ ] **Step 5: 验证编译并提交**

```bash
cargo build 2>&1 | grep "^error" | head -10
git add src/core/platform/unix/terminal.rs src/core/platform/windows/terminal.rs \
        src/core/platform/unix/mod.rs src/core/platform/mod.rs \
        src/core/terminal/size.rs
git commit -m "refactor: deduplicate terminal_size, delegate to platform/"
```

---

### Task 7c：`PlatformReactor` 补全接口，消除 `terminal/events.rs` 中的 epoll 直调

`events.rs` 需要：① 向 reactor 注册指定 token 的 fd；② 等待事件并返回 token 列表。这两个操作应是 `PlatformReactor` 的公开接口，而非 `events.rs` 自己调 epoll FFI。

**Files:**
- Modify: `src/core/platform/unix/reactor.rs` — 添加 `register_fd` / `wait_tokens`
- Modify: `src/core/platform/windows/reactor.rs` — 添加 stub
- Modify: `src/core/terminal/events.rs` — 删除 epoll FFI，改调 reactor 方法

- [ ] **Step 1: 在 `src/core/platform/unix/reactor.rs` 添加两个方法**

```rust
/// Register a raw fd with a caller-specified token (for event multiplexing).
/// Returns Ok(true) on success, Ok(false) if EPERM (fd not epoll-able, e.g. /dev/null).
pub fn register_fd(&self, fd: RawHandle, token: u64) -> crate::Result<bool> {
    let mut ev = EpollEvent { events: EPOLLIN, data: token };
    let ret = unsafe { epoll_ctl(self.epfd, EPOLL_CTL_ADD, fd, &mut ev) };
    if ret == 0 {
        return Ok(true);
    }
    let errno = unsafe { *__errno_location() };
    const EPERM: i32 = 1;
    if errno == EPERM {
        return Ok(false);
    }
    Err(crate::Error::Io(std::io::Error::from_raw_os_error(errno)))
}

/// Wait for events, returning a list of fired tokens. Empty on timeout or EINTR.
pub fn wait_tokens(&self, timeout_ms: i32) -> crate::Result<Vec<u64>> {
    const MAX: usize = 64;
    let mut events = [EpollEvent { events: 0, data: 0 }; MAX];
    let n = unsafe { epoll_wait(self.epfd, events.as_mut_ptr(), MAX as i32, timeout_ms) };
    if n < 0 {
        let errno = unsafe { *__errno_location() };
        if errno == EINTR {
            return Ok(Vec::new());
        }
        return Err(crate::Error::Io(std::io::Error::from_raw_os_error(errno)));
    }
    Ok(events[..n as usize].iter().map(|e| e.data).collect())
}
```

- [ ] **Step 2: 在 `src/core/platform/windows/reactor.rs` 添加 stub**

```rust
pub fn register_fd(&self, _fd: RawHandle, _token: u64) -> crate::Result<bool> {
    Ok(false) // TODO: IOCP implementation
}

pub fn wait_tokens(&self, _timeout_ms: i32) -> crate::Result<Vec<u64>> {
    Ok(Vec::new()) // TODO: IOCP implementation
}
```

- [ ] **Step 3: 更新 `src/core/terminal/events.rs`**

删除文件顶部所有 `#[cfg(unix)]` 的 FFI 声明（`epoll_ctl`, `__errno_location`, `EpollEventRaw`, `EPOLL_CTL_ADD`, `EPOLLIN`, `EPERM` 常量，以及 `epoll_try_add` 函数）。

在 `EventLoop::new()` 中，将：
```rust
#[cfg(unix)]
{
    let epoll_fd = reactor.epoll_fd();
    let input_fd = terminal.input_handle();
    stdin_in_epoll = epoll_try_add(epoll_fd, input_fd, TOKEN_STDIN)?;
    let mut ev = EpollEventRaw { events: EPOLLIN, data: TOKEN_SIGNAL };
    let ret = unsafe { epoll_ctl(epoll_fd, EPOLL_CTL_ADD, resize.handle(), &mut ev) };
    if ret < 0 { return Err(...); }
}
#[cfg(windows)]
{ stdin_in_epoll = false; }
```
改为：
```rust
let input_fd = terminal.input_handle();
stdin_in_epoll = reactor.register_fd(input_fd, TOKEN_STDIN)?;
reactor.register_fd(resize.handle(), TOKEN_SIGNAL)?;
```

在 `wait_events` 中，将整个 `#[cfg(unix)] { ... } #[cfg(windows)] { ... }` 块改为：
```rust
fn wait_events(&self, timeout_ms: i32) -> crate::Result<Vec<u64>> {
    self.reactor.wait_tokens(timeout_ms)
}
```

- [ ] **Step 4: 验证编译并提交**

```bash
cargo build 2>&1 | grep "^error" | head -20
git add src/core/platform/unix/reactor.rs src/core/platform/windows/reactor.rs \
        src/core/terminal/events.rs
git commit -m "refactor: expose register_fd/wait_tokens on PlatformReactor, remove epoll FFI from events.rs"
```

---

### Task 7d：子进程 + pipe 抽象，消除 `lsp/` 和 `mcp/` 中的 cfg

`lsp/` 和 `mcp/transport/stdio.rs` 需要启动子进程并通过 pipe 通信。`platform` 目前只有 `shell_command()` 返回 `Command`，没有封装 stdin/stdout pipe 的 `ChildProcess`。

**Files:**
- Modify: `src/core/platform/unix/process.rs` — 添加 `ChildProcess` + `spawn_piped`
- Modify: `src/core/platform/windows/process.rs` — 添加 stub
- Modify: `src/core/platform/types.rs` — 添加 `ChildProcess` 类型（或在 process.rs 定义后 re-export）
- Modify: `src/core/platform/mod.rs` — re-export `ChildProcess`, `spawn_piped`
- Modify: `src/mcp/transport/stdio.rs` — 使用 `platform::spawn_piped`，删除 `unix_io::stdin_fd/stdout_fd`
- Modify: `src/lsp/mod.rs` — 使用 `platform::spawn_piped`

- [ ] **Step 1: 扩展 `src/core/platform/unix/process.rs`**

```rust
use std::process::{Command, Stdio};
use std::os::unix::io::AsRawFd;
use crate::core::platform::types::RawHandle;

pub fn shell_command(cmd: &str) -> Command {
    let mut c = Command::new("sh");
    c.arg("-c").arg(cmd);
    c
}

/// A spawned child process with accessible stdin/stdout raw handles.
pub struct ChildProcess {
    pub child: std::process::Child,
    pub stdin_fd: RawHandle,
    pub stdout_fd: RawHandle,
}

/// Spawn a process with piped stdin/stdout. stderr is discarded.
pub fn spawn_piped(cmd: &str, args: &[&str]) -> crate::Result<ChildProcess> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(crate::Error::Io)?;
    let stdin_fd = child.stdin.as_ref()
        .ok_or_else(|| crate::Error::Invariant("spawn_piped: no stdin".into()))?
        .as_raw_fd();
    let stdout_fd = child.stdout.as_ref()
        .ok_or_else(|| crate::Error::Invariant("spawn_piped: no stdout".into()))?
        .as_raw_fd();
    Ok(ChildProcess { child, stdin_fd, stdout_fd })
}
```

- [ ] **Step 2: 扩展 `src/core/platform/windows/process.rs`**

```rust
use std::process::Command;
use crate::core::platform::types::RawHandle;

pub fn shell_command(cmd: &str) -> Command {
    let mut c = Command::new("cmd");
    c.args(["/C", cmd]);
    c
}

pub struct ChildProcess {
    pub child: std::process::Child,
    pub stdin_fd: RawHandle,
    pub stdout_fd: RawHandle,
}

pub fn spawn_piped(_cmd: &str, _args: &[&str]) -> crate::Result<ChildProcess> {
    Err(crate::Error::Invariant("spawn_piped not yet implemented on Windows".into()))
}
```

- [ ] **Step 3: 在 `src/core/platform/mod.rs` 添加 re-export**

```rust
#[cfg(unix)]
pub use unix::process::{ChildProcess, spawn_piped};
#[cfg(windows)]
pub use windows::process::{ChildProcess, spawn_piped};
```

- [ ] **Step 4: 更新 `src/mcp/transport/stdio.rs`**

在 `StdioTransport::spawn` 或等效位置，将：
```rust
use std::process::{Child, Command, Stdio};
// ... #[cfg(unix)] use ... AsRawFd
let child = Command::new(cmd).args(args)
    .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
    .spawn()?;
let stdin_fd = unix_io::stdin_fd(&child).ok_or(...)?;
let stdout_fd = unix_io::stdout_fd(&child).ok_or(...)?;
```
改为：
```rust
use crate::core::platform;
let proc = platform::spawn_piped(cmd, args)?;
let stdin_fd = proc.stdin_fd;
let stdout_fd = proc.stdout_fd;
// proc.child 由 StdioTransport 持有以维持生命周期
```

删除 `unix_io` 中的 `stdin_fd`/`stdout_fd` 函数（`read`/`write`/`set_nonblocking` 可保留，它们是 Unix 内部实现细节，不含 cfg 分支）。

- [ ] **Step 5: 更新 `src/lsp/mod.rs` 中的进程启动代码**

将：
```rust
#[cfg(unix)]
let child = Command::new(&server_cfg.command)
    .args(&server_cfg.args)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .spawn()?;
#[cfg(unix)]
let stdin_fd = child.stdin.as_ref()...as_raw_fd();
```
改为：
```rust
let proc = crate::core::platform::spawn_piped(
    &server_cfg.command,
    &server_cfg.args.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
)?;
```

- [ ] **Step 6: 全量验证**

```bash
cargo build 2>&1 | grep "^error" | head -20
cargo test 2>&1 | tail -20
```

- [ ] **Step 7: Commit**

```bash
git add src/core/platform/unix/process.rs src/core/platform/windows/process.rs \
        src/core/platform/mod.rs \
        src/mcp/transport/stdio.rs src/lsp/mod.rs
git commit -m "refactor: add platform::spawn_piped+ChildProcess, remove cfg from lsp/mcp process spawning"
```

---

## 最终验证

所有任务完成后：

```bash
cargo build --release
cargo test
cargo clippy -- -D warnings
```

期望：全部通过，无新增 warning。
