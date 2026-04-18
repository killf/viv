# LSP 文件同步实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 EditTool/WriteTool/MultiEdit 执行成功后，立即通知 LSP server 文件已变更，使 LSP 缓存失效，下一次 LSP 查询返回最新结果。

**Architecture:**
- `LspClient` 新增 `notify_did_change` 方法，发送 `textDocument/didChange` JSON-RPC notification
- `LspManager` 新增 `OpenDocument` 跟踪每个文件的版本，`ensure_file_open` 初始化 version=1，`notify_did_change` 递增版本并通知对应 client
- `Agent::agentic_loop` 在工具执行结果收集处拦截 FileEdit/FileWrite/MultiEdit，从返回字符串提取文件路径，调用 `lsp_manager.notify_did_change`

**Tech Stack:** Rust (no new dependencies), custom JSON-RPC, custom async runtime

---

## 文件变更概览

| 文件 | 改动 |
|------|------|
| `src/lsp/client.rs` | 新增 `notify_did_change` 方法 |
| `src/lsp/mod.rs` | 新增 `OpenDocument` struct、`open_documents` HashMap、版本跟踪、`did_open` 初始化 version |
| `src/agent/agent.rs` | `agentic_loop` 中拦截写文件工具，提取路径，调用 `lsp_manager.notify_did_change` |
| `tests/lsp/client_test.rs` | 新增 `did_change_notification` 测试 |
| `tests/lsp/manager_test.rs` | 新增 `notify_did_change` 测试 |

---

## Task 1: LspClient 新增 `notify_did_change`

**Files:** Modify: `src/lsp/client.rs:155-178`（在 `did_open` 方法之后新增）

- [ ] **Step 1: 新增 `notify_did_change` 方法**

在 `src/lsp/client.rs` 的 `LspClient` impl 块中，`shutdown` 方法之前添加：

```rust
/// Send a `textDocument/didChange` notification with full file content.
pub async fn notify_did_change(
    &mut self,
    uri: &str,
    content: &str,
    version: i32,
) -> crate::Result<()> {
    let params = JsonValue::Object(vec![(
        "textDocument".to_string(),
        JsonValue::Object(vec![
            ("uri".to_string(), JsonValue::Str(uri.to_string())),
            (
                "version".to_string(),
                JsonValue::Number(crate::core::json::Number::Int(version as i64)),
            ),
        ]),
    ), (
        "contentChanges".to_string(),
        JsonValue::Array(vec![
            JsonValue::Object(vec![
                ("text".to_string(), JsonValue::Str(content.to_string())),
            ]),
        ]),
    )]);
    self.notify("textDocument/didChange", Some(params)).await
}
```

- [ ] **Step 2: 运行测试确认编译通过**

Run: `cargo test --test llm_test lsp::client_test::did_open_notification -- --nocapture`（或跳过，实际用 `cargo build` 验证）
Run: `cargo build 2>&1 | head -30`
Expected: 无编译错误

- [ ] **Step 3: 提交**

```bash
git add src/lsp/client.rs
git commit -m "feat(lsp): add notify_did_change to LspClient

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 2: LspManager 版本跟踪 + notify_did_change

**Files:** Modify: `src/lsp/mod.rs`

- [ ] **Step 1: 添加 OpenDocument struct**

在 `LspManager` 定义之前添加：

```rust
/// Tracks an open document's version and the LSP client that owns it.
struct OpenDocument {
    /// Server name this document is open in.
    server_name: String,
    /// Monotonically increasing version. Starts at 1 on `didOpen`, incremented on each `didChange`.
    version: i32,
}
```

- [ ] **Step 2: 修改 LspManager 字段**

在 `LspManager` 中：
- 删除 `opened_files: HashSet<String>`（HashSet 不够用）
- 新增 `open_documents: HashMap<String, OpenDocument>`（key = 文件路径/URI）
- 删除构造函数 `LspManager::new` 中的 `opened_files: HashSet::new()` 初始化

```rust
// 替换 opened_files: HashSet<String> 为：
open_documents: HashMap<String, OpenDocument>,
```

```rust
// 构造函数中替换为：
open_documents: HashMap::new(),
```

- [ ] **Step 3: 修改 `ensure_file_open` 发送 did_open + 初始化版本**

找到 `ensure_file_open` 方法，修改 body：

```rust
pub async fn ensure_file_open(&mut self, file: &str) -> Result<()> {
    let uri = path_to_uri(file);
    if self.open_documents.contains_key(&uri) {
        return Ok(());
    }

    let content = std::fs::read_to_string(file).map_err(Error::Io)?;
    let lang = language_id_from_path(file).to_string();

    let client = self.get_or_start(file).await?;
    client.did_open(&uri, &lang, &content).await?;

    // Record the open document with version=1.
    let server_name = self.server_name_for_file(file).unwrap().to_string();
    self.open_documents.insert(uri, OpenDocument { server_name, version: 1 });
    Ok(())
}
```

注意：`let uri = path_to_uri(file);` 要移到前面（因为 `get_or_start` 用的是原始 file path 字符串），同时要处理可能的错误。

- [ ] **Step 4: 新增 `notify_did_change` 方法**

在 `LspManager impl` 块中，`shutdown_all` 之后添加：

```rust
/// Notify the appropriate LSP server that a file has changed.
///
/// Reads the current file content from disk, increments the version, and sends
/// `textDocument/didChange`. If the file has not been opened via `ensure_file_open`
/// this is a no-op.
pub async fn notify_did_change(&mut self, file: &str) -> Result<()> {
    let uri = path_to_uri(file);

    let entry = match self.open_documents.get_mut(&uri) {
        Some(e) => e,
        None => return Ok(()), // file not open, nothing to do
    };

    // Read updated content from disk.
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("failed to read changed file '{}': {}", file, e);
            return Ok(());
        }
    };

    // Get or start the server.
    let client = self.get_or_start(file).await?;

    // Increment version before sending.
    entry.version += 1;
    let version = entry.version;

    client.notify_did_change(&uri, &content, version).await?;
    Ok(())
}
```

- [ ] **Step 5: 修改 `shutdown_all` 清理 open_documents**

将 `self.opened_files.clear()` 改为 `self.open_documents.clear()`。

- [ ] **Step 6: 运行测试确认编译**

Run: `cargo build 2>&1 | head -40`
Expected: 无编译错误

- [ ] **Step 7: 提交**

```bash
git add src/lsp/mod.rs
git commit -m "feat(lsp): track per-file version and implement notify_did_change

- Add OpenDocument struct with version tracking
- Replace opened_files HashSet with open_documents HashMap
- ensure_file_open now initializes version=1
- Add LspManager::notify_did_change for file change notifications

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 3: Agent 层拦截写文件工具

**Files:** Modify: `src/agent/agent.rs`

- [ ] **Step 1: 在 `agentic_loop` 工具结果收集处添加 LSP 通知逻辑**

在 `src/agent/agent.rs` 的 `agentic_loop` 方法中，找到工具执行结果收集的循环（约第 270-312 行），在 `tool_results.push(...)` 之后、循环结束后，添加：

在 `ContentBlock::ToolResult { ... }` push 之后、`} // for tu in &tool_uses` 之后，添加：

```rust
// After all tool results are collected, notify LSP of any file changes.
for tu in &tool_uses {
    if let ContentBlock::ToolUse { name, input, .. } = tu {
        if matches!(name.as_str(), "FileEdit" | "FileWrite" | "MultiEdit") {
            let path = input
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            if let Some(path) = path {
                let mut mgr = self.lsp.lock().unwrap();
                // Safe: single-threaded runtime, guard does not cross threads.
                #[allow(clippy::await_holding_lock)]
                if let Err(e) = mgr.notify_did_change(&path).await {
                    tracing::warn!("failed to notify LSP of file change: {}", e);
                }
            }
        }
    }
}
```

- [ ] **Step 2: 确认编译**

Run: `cargo build 2>&1 | head -50`
Expected: 无编译错误

注意：`Arc` 在 `agent.rs` 中已导入（`use std::sync::{Arc, Mutex};`），无需新增 import。

- [ ] **Step 3: 提交**

```bash
git add src/agent/agent.rs
git commit -m "feat(agent): notify LSP of file changes after edit/write tools

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 4: 新增测试

**Files:** Modify: `tests/lsp/client_test.rs`, `tests/lsp/manager_test.rs`

- [ ] **Step 1: `client_test.rs` 新增 `did_change_notification` 测试**

在 `tests/lsp/client_test.rs` 末尾（在最后一个测试之后）添加：

```rust
/// `notify_did_change` sends a notification (no id) with correct params.
#[test]
fn did_change_notification() {
    let transport = MockTransport::new(vec![]);
    let mut client = LspClient::new(transport, "rust-analyzer");

    let client = block_on(async move {
        client
            .notify_did_change("file:///src/main.rs", "fn updated() {}", 2)
            .await
            .unwrap();
        client
    });

    let transport = client.into_transport();
    assert_eq!(transport.sent.len(), 1);
    let msg = &transport.sent[0];
    assert_eq!(msg.get("method").unwrap().as_str().unwrap(), "textDocument/didChange");
    assert!(msg.get("id").is_none(), "didChange must NOT have an id");

    let params = msg.get("params").unwrap();
    let doc = params.get("textDocument").unwrap();
    assert_eq!(doc.get("uri").unwrap().as_str().unwrap(), "file:///src/main.rs");
    assert_eq!(doc.get("version").unwrap().as_i64().unwrap(), 2);

    let changes = params.get("contentChanges").unwrap().as_array().unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].get("text").unwrap().as_str().unwrap(), "fn updated() {}");
}
```

- [ ] **Step 2: `manager_test.rs` 新增 `notify_did_change` 测试**

在 `tests/lsp/manager_test.rs` 末尾添加：

```rust
// ---------------------------------------------------------------------------
// notify_did_change
// ---------------------------------------------------------------------------

// Note: Full integration test with real LSP server is under tests/lsp/integration_test.rs.
// Unit test here verifies path extraction and no-op for unopened files.
```

由于 `LspManager` 的 `notify_did_change` 需要实际启动 LSP server（调用 `get_or_start`），在 `manager_test.rs` 中测试它需要 mock `StdioTransport::spawn_with_framing`。这比较复杂，可以将完整的集成测试留给 `integration_test.rs`，或者添加一个简单的测试验证：当文件不在 `open_documents` 中时 `notify_did_change` 是 no-op。

如果要做 no-op 测试：创建一个不含任何 server 的空 config 的 `LspManager`，调用 `notify_did_change`，验证不报错：

```rust
#[test]
fn notify_did_change_noop_for_unopened_file() {
    let config = LspConfig::parse("{}").unwrap();
    let mut manager = LspManager::new(config);

    // Should be a no-op, not an error.
    let result = viv::core::runtime::block_on(
        manager.notify_did_change("/tmp/does_not_exist.rs")
    );
    assert!(result.is_ok());
}
```

- [ ] **Step 3: 运行测试**

Run: `cargo test --test llm_test lsp::client_test::did_change_notification -- --nocapture`
Expected: PASS

Run: `cargo test --test llm_test lsp::manager_test::notify_did_change_noop_for_unopened_file -- --nocapture`
Expected: PASS

- [ ] **Step 4: 提交**

```bash
git add tests/lsp/client_test.rs tests/lsp/manager_test.rs
git commit -m "test(lsp): add didChange notification tests

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Task 5: 验证

- [ ] **Step 1: 运行所有测试**

Run: `cargo test 2>&1 | tail -20`
Expected: 所有测试 PASS

- [ ] **Step 2: 运行 clippy**

Run: `cargo clippy 2>&1 | grep -E "(error|warning)" | head -20`
Expected: 无 error，只有允许的 warning

- [ ] **Step 3: 格式化**

Run: `cargo fmt`
Expected: 无差异（或仅格式化本次改动文件）

---

## Task 6: 最终提交

如果所有步骤通过，执行一次汇总提交（如果 Task 1-4 的提交已分步完成，可跳过此步）：

```bash
git log --oneline -6
```