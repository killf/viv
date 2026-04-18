# LSP 文件同步设计规格

**日期**: 2026-04-18
**状态**: 已批准
**关联**: `src/lsp/`, `src/agent/`, `src/tools/file/`

## 背景

Agent 通过 EditTool/WriteTool 修改文件后，LSP server 的缓存不会自动更新，导致：
- Hover 工具返回旧的文档/类型信息
- Diagnostics 工具返回过期的诊断结果
- Definition/References 可能指向错误位置

## 目标

在 EditTool/WriteTool 执行成功后，立即通知 LSP server 文件已变更，使 LSP 缓存失效，下一次 LSP 查询返回最新结果。

## 设计决策

- **同步时机**: 工具执行后立即同步，不批量不 debounce
- **通知内容**: 发送完整文件内容（简化实现，可靠性优先）
- **拦截位置**: Agent 层拦截工具名，不在工具层引入 LSP 依赖

## 架构

```
Agent::agentic_loop()
  └── 工具执行完成（EditTool / WriteTool / MultiEditTool）
          │
          ▼
  检查工具名
          │
          ▼
  提取返回结果中的文件路径
          │
          ▼
  lsp_manager.notify_did_change(path)
          │
          ├── 查找 path 对应的已打开文件的 version
          ├── 读取文件完整内容（std::fs）
          ├── 调用 lsp_client.notify_did_change(...)
          └── version += 1
```

## 改动详解

### 1. LspClient 新增 `notify_did_change`

**文件**: `src/lsp/client.rs`

```rust
pub async fn notify_did_change(
    &mut self,
    uri: &str,
    content: &str,
    version: i32,
) -> Result<()>
```

发送 `textDocument/didChange` JSON-RPC notification（无 response）：

```json
{
  "jsonrpc": "2.0",
  "method": "textDocument/didChange",
  "params": {
    "textDocument": {
      "uri": "file:///path/to/file.rs",
      "version": 3
    },
    "contentChanges": [
      { "text": "full file content here..." }
    ]
  }
}
```

超时不等待 response，发送后立即返回。

### 2. LspManager 新增 `notify_did_change`

**文件**: `src/lsp/mod.rs`

新增数据结构跟踪每个文件的版本：

```rust
struct OpenDocument {
    client_name: String,
    version: i32,
}
```

新增方法：

```rust
pub async fn notify_did_change(&mut self, path: &Path) -> Result<()>
```

流程：
1. `path_to_uri(path)` 转换路径
2. 在 `open_documents` 中查找对应 entry
3. 读取文件内容（`tokio::fs::read_to_string`）
4. 找到对应 `LspClient`
5. 调用 `client.notify_did_change(uri, content, version)`
6. `version += 1` 更新

### 3. LspManager 新增 `did_open` 通知

**文件**: `src/lsp/mod.rs`

在 `LspManager::open_document` 中，文件首次打开时发送 `textDocument/didOpen`：

```json
{
  "jsonrpc": "2.0",
  "method": "textDocument/didOpen",
  "params": {
    "textDocument": {
      "uri": "file:///path/to/file.rs",
      "languageId": "rust",
      "version": 1,
      "text": "..."
    }
  }
}
```

初始化 `open_documents` entry，version 从 1 开始。

### 4. Agent 层拦截

**文件**: `src/agent/agent.rs`

在 `agentic_loop` 的工具执行结果收集处添加：

```rust
// 检查是否是写文件类工具
let is_write_tool = matches!(
    tool.name().as_str(),
    "edit" | "write" | "multi_edit"
);

let result_str = tool.execute(...).await?;

if is_write_tool {
    // 从结果中提取路径并通知 LSP
    if let Some(path) = extract_path_from_result(&result_str) {
        if let Err(e) = self.lsp_manager.notify_did_change(&path).await {
            tracing::warn!("failed to notify LSP: {}", e);
        }
    }
}
```

`extract_path_from_result` 从工具返回的 JSON 中提取 `"path"` 字段。

### 5. `didClose` 支持（可选，v2）

未来在文件关闭时发送 `textDocument/didClose` 清理 `open_documents`，本次实现不包含。

## 错误处理

| 场景 | 处理 |
|------|------|
| LSP server 不支持 didChange | 超时 3s 忽略，继续执行 |
| 文件不存在或无法读取 | `tracing::warn` 记录，跳过 |
| 找不到对应 LSP client | 无操作，不报错 |
| open_documents 中无该文件记录 | 无操作（server 未主动追踪） |

## 文件变更清单

| 文件 | 改动 |
|------|------|
| `src/lsp/client.rs` | 新增 `notify_did_change` 方法 |
| `src/lsp/mod.rs` | 新增 `OpenDocument` 结构、`open_documents` HashMap、`did_open` 通知、`notify_did_change` 方法、`path_to_uri` 公开 |
| `src/agent/agent.rs` | `agentic_loop` 中拦截写文件工具，调用 `lsp_manager.notify_did_change` |
| `tests/lsp/` | 新增 `did_change` 集成测试 |

## 测试策略

- **单元测试**: 构造 MockTransport，验证 `notify_did_change` 发送正确的 JSON-RPC notification
- **集成测试**: `full_test` feature 下连接真实 rust-analyzer，验证诊断结果在编辑后更新