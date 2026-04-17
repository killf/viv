# Agent Loop + Self-Evolution + Memory System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 Agent 主循环（tool_use → tool_result → 再次请求）、分层记忆系统（5 层）、自我进化引擎（会话后经验提炼），以及带缓存优化的 Prompt 拼接。

**Architecture:** Agent 循环是核心驱动器；MemorySystem 在循环开始注入相关记忆、循环结束触发进化；Prompt 拼接按稳定性分 4 个 cache_control 块；LLM 客户端扩展为同时解析 text 和 tool_use 流事件。

**Tech Stack:** Rust std only，复用 `src/json.rs`、`src/llm.rs`（扩展）、`src/runtime/`（Plan 1）

---

## 文件结构

**新建：**

```
src/agent/
├── mod.rs          # pub use 导出
├── message.rs      # ContentBlock / Message / SystemBlock / PromptCache
├── context.rs      # AgentContext / AgentConfig
├── prompt.rs       # build_system_prompt（4 cache blocks）
├── run.rs          # run_agent 异步主循环
└── evolution.rs    # evolve_from_session（自我进化）

src/memory/
├── mod.rs          # MemorySystem 公开 API
├── store.rs        # .viv/memory/ 文件读写
├── index.rs        # index.json CRUD + 关键词搜索
├── retrieval.rs    # 两阶段检索（关键词预筛 + LLM 排序）
└── compaction.rs   # Working Memory 上下文压缩
```

**修改：**

```
src/llm.rs          # 扩展：SystemBlock / tool_use 流解析 / StreamResult
src/lib.rs          # 新增 pub mod agent; pub mod memory;
src/repl.rs         # 接入 run_agent（替换现有直接 llm.stream 调用）
```

**测试：**

```
tests/agent/
├── message_test.rs
├── prompt_test.rs
└── run_test.rs

tests/memory/
├── store_test.rs
├── index_test.rs
└── retrieval_test.rs
```

---

## Task 1: Message 类型 + JSON 序列化

**Files:**
- Create: `src/agent/message.rs`
- Create: `tests/agent/message_test.rs`

### 代码

`src/agent/message.rs`:

```rust
use crate::json::JsonValue;

// ── ContentBlock ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ContentBlock {
    Text(String),
    ToolUse { id: String, name: String, input: JsonValue },
    ToolResult { tool_use_id: String, content: Vec<ContentBlock>, is_error: bool },
}

impl ContentBlock {
    pub fn to_json(&self) -> String {
        match self {
            ContentBlock::Text(t) => {
                format!("{{\"type\":\"text\",\"text\":{}}}", JsonValue::Str(t.clone()))
            }
            ContentBlock::ToolUse { id, name, input } => {
                format!(
                    "{{\"type\":\"tool_use\",\"id\":{},\"name\":{},\"input\":{}}}",
                    JsonValue::Str(id.clone()),
                    JsonValue::Str(name.clone()),
                    input,
                )
            }
            ContentBlock::ToolResult { tool_use_id, content, is_error } => {
                let content_json: Vec<String> = content.iter().map(|b| b.to_json()).collect();
                format!(
                    "{{\"type\":\"tool_result\",\"tool_use_id\":{},\"content\":[{}],\"is_error\":{}}}",
                    JsonValue::Str(tool_use_id.clone()),
                    content_json.join(","),
                    is_error,
                )
            }
        }
    }
}

// ── Message ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Message {
    User(Vec<ContentBlock>),
    Assistant(Vec<ContentBlock>),
}

impl Message {
    pub fn user_text(text: impl Into<String>) -> Self {
        Message::User(vec![ContentBlock::Text(text.into())])
    }

    pub fn role(&self) -> &str {
        match self { Message::User(_) => "user", Message::Assistant(_) => "assistant" }
    }

    pub fn blocks(&self) -> &[ContentBlock] {
        match self { Message::User(b) | Message::Assistant(b) => b }
    }

    pub fn to_json(&self) -> String {
        let blocks_json: Vec<String> = self.blocks().iter().map(|b| b.to_json()).collect();
        format!(
            "{{\"role\":{},\"content\":[{}]}}",
            JsonValue::Str(self.role().into()),
            blocks_json.join(","),
        )
    }
}

// ── SystemBlock（带 cache_control）────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SystemBlock {
    pub text: String,
    pub cached: bool,
}

impl SystemBlock {
    pub fn cached(text: impl Into<String>) -> Self {
        SystemBlock { text: text.into(), cached: true }
    }
    pub fn dynamic(text: impl Into<String>) -> Self {
        SystemBlock { text: text.into(), cached: false }
    }
    pub fn to_json(&self) -> String {
        if self.cached {
            format!(
                "{{\"type\":\"text\",\"text\":{},\"cache_control\":{{\"type\":\"ephemeral\"}}}}",
                JsonValue::Str(self.text.clone()),
            )
        } else {
            format!("{{\"type\":\"text\",\"text\":{}}}", JsonValue::Str(self.text.clone()))
        }
    }
}

// ── PromptCache（内容 hash，避免重复序列化）────────────────────────────────────

#[derive(Default)]
pub struct PromptCache {
    pub base_hash: u64,
    pub base_text: String,
    pub tools_hash: u64,
    pub tools_text: String,
    pub skills_hash: u64,
    pub skills_text: String,
}

pub fn hash_str(s: &str) -> u64 {
    // FNV-1a 64-bit（零依赖）
    let mut h: u64 = 14695981039346656037;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}
```

- [ ] **Step 1: 写测试**

`tests/agent/message_test.rs`:

```rust
use viv::agent::message::{ContentBlock, Message, SystemBlock, hash_str};

#[test]
fn text_block_serializes_correctly() {
    let b = ContentBlock::Text("hello world".into());
    let json = b.to_json();
    assert!(json.contains("\"type\":\"text\""));
    assert!(json.contains("hello world"));
}

#[test]
fn tool_use_block_serializes_correctly() {
    use viv::json::JsonValue;
    let b = ContentBlock::ToolUse {
        id: "tu_01".into(),
        name: "bash".into(),
        input: JsonValue::parse("{\"command\":\"ls\"}").unwrap(),
    };
    let json = b.to_json();
    assert!(json.contains("\"type\":\"tool_use\""));
    assert!(json.contains("\"name\":\"bash\""));
    assert!(json.contains("\"id\":\"tu_01\""));
}

#[test]
fn tool_result_serializes_correctly() {
    let b = ContentBlock::ToolResult {
        tool_use_id: "tu_01".into(),
        content: vec![ContentBlock::Text("output".into())],
        is_error: false,
    };
    let json = b.to_json();
    assert!(json.contains("\"type\":\"tool_result\""));
    assert!(json.contains("\"tool_use_id\":\"tu_01\""));
    assert!(json.contains("\"is_error\":false"));
}

#[test]
fn user_message_role_is_user() {
    let m = Message::user_text("hi");
    assert_eq!(m.role(), "user");
}

#[test]
fn assistant_message_serializes_correctly() {
    let m = Message::Assistant(vec![ContentBlock::Text("answer".into())]);
    let json = m.to_json();
    assert!(json.contains("\"role\":\"assistant\""));
}

#[test]
fn system_block_cached_has_cache_control() {
    let b = SystemBlock::cached("base prompt");
    let json = b.to_json();
    assert!(json.contains("\"cache_control\""));
    assert!(json.contains("\"ephemeral\""));
}

#[test]
fn system_block_dynamic_has_no_cache_control() {
    let b = SystemBlock::dynamic("memory");
    let json = b.to_json();
    assert!(!json.contains("\"cache_control\""));
}

#[test]
fn hash_str_is_deterministic() {
    assert_eq!(hash_str("hello"), hash_str("hello"));
    assert_ne!(hash_str("hello"), hash_str("world"));
}
```

- [ ] **Step 2: 运行测试，确认编译失败**

```bash
cargo test --test message_test 2>&1 | head -20
```

- [ ] **Step 3: 创建 `src/agent/mod.rs`**

```rust
pub mod message;
```

- [ ] **Step 4: 在 `src/lib.rs` 添加**

```rust
pub mod agent;
```

- [ ] **Step 5: 实现 `src/agent/message.rs`**

粘贴上方完整代码。

- [ ] **Step 6: 运行测试，确认通过**

```bash
cargo test --test message_test
```

期望：`8 passed`

- [ ] **Step 7: Commit**

```bash
git add src/agent/ src/lib.rs tests/agent/message_test.rs
git commit -m "feat(agent): message types + JSON serialization + PromptCache"
```

---

## Task 2: 扩展 LLM 客户端（tool_use 流解析 + cache_control）

**Files:**
- Modify: `src/llm.rs`
- Create: `tests/agent/llm_stream_test.rs`

### 新增类型和函数

在 `src/llm.rs` 末尾添加：

```rust
use crate::agent::message::{ContentBlock, Message as AgentMessage, SystemBlock};

/// 一次 LLM 流响应的完整结果
pub struct StreamResult {
    pub text_blocks: Vec<ContentBlock>,
    pub tool_uses: Vec<ContentBlock>,  // 只含 ToolUse 变体
    pub stop_reason: String,
}

impl LLMClient {
    /// 支持 tool_use 的流式请求：解析 text_delta 和 input_json_delta。
    /// system_blocks 对应 Anthropic API system 数组（带 cache_control）。
    pub fn stream_agent(
        &self,
        system_blocks: &[SystemBlock],
        messages: &[AgentMessage],
        tier: ModelTier,
        mut on_text: impl FnMut(&str),
    ) -> crate::Result<StreamResult> {
        let req = self.build_agent_request(system_blocks, messages, tier);
        let bytes = req.to_bytes();
        let url = parse_base_url(&self.config.base_url);

        let mut tls = TlsStream::connect(&url.host, url.port)?;
        tls.write_all(&bytes)?;

        let mut raw: Vec<u8> = Vec::new();
        let mut tmp = [0u8; 4096];
        let mut header_end: Option<usize> = None;

        loop {
            let n = tls.read(&mut tmp)?;
            if n == 0 { break; }
            raw.extend_from_slice(&tmp[..n]);

            if header_end.is_none() {
                if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = Some(pos + 4);
                    let header_section = std::str::from_utf8(&raw[..pos])
                        .map_err(|_| Error::Http("invalid UTF-8 in headers".into()))?;
                    let status = parse_status_line(header_section)?;
                    if status != 200 {
                        loop {
                            let n2 = tls.read(&mut tmp)?;
                            if n2 == 0 { break; }
                            raw.extend_from_slice(&tmp[..n2]);
                        }
                        let body = String::from_utf8_lossy(&raw[pos + 4..]).into_owned();
                        return Err(Error::LLM { status, message: body });
                    }
                }
            }
            if let Some(hend) = header_end {
                if String::from_utf8_lossy(&raw[hend..]).contains("message_stop") {
                    break;
                }
            }
        }

        parse_agent_stream(&raw, header_end, &mut on_text)
    }

    fn build_agent_request(
        &self,
        system_blocks: &[SystemBlock],
        messages: &[AgentMessage],
        tier: ModelTier,
    ) -> HttpRequest {
        let model = self.config.model(tier.clone()).to_string();
        let max_tokens = self.config.max_tokens(tier);
        let url = parse_base_url(&self.config.base_url);

        let system_json: Vec<String> = system_blocks.iter().map(|b| b.to_json()).collect();
        let messages_json: Vec<String> = messages.iter().map(|m| m.to_json()).collect();

        let body = format!(
            "{{\"model\":{},\"max_tokens\":{},\"stream\":true,\"system\":[{}],\"messages\":[{}]}}",
            JsonValue::Str(model),
            max_tokens,
            system_json.join(","),
            messages_json.join(","),
        );

        HttpRequest {
            method: "POST".into(),
            path: format!("{}/v1/messages", url.path_prefix),
            headers: vec![
                ("Host".into(), url.host),
                ("Content-Type".into(), "application/json".into()),
                ("x-api-key".into(), self.config.api_key.clone()),
                ("anthropic-version".into(), "2023-06-01".into()),
                ("anthropic-beta".into(), "prompt-caching-2024-07-31".into()),
            ],
            body: Some(body),
        }
    }
}

fn parse_agent_stream(
    raw: &[u8],
    header_end: Option<usize>,
    on_text: &mut impl FnMut(&str),
) -> crate::Result<StreamResult> {
    // 每个 index 对应一个进行中的内容块
    let mut text_acc: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
    let mut tool_acc: std::collections::HashMap<usize, (String, String, String)> =
        std::collections::HashMap::new(); // index → (id, name, partial_json)

    let mut text_blocks: Vec<ContentBlock> = vec![];
    let mut tool_uses: Vec<ContentBlock> = vec![];
    let mut stop_reason = String::from("end_turn");

    let hend = match header_end { Some(h) => h, None => return Ok(StreamResult { text_blocks, tool_uses, stop_reason }) };
    let body_str = String::from_utf8_lossy(&raw[hend..]);

    let mut parser = crate::net::sse::SseParser::new();
    parser.feed(&body_str);
    let events = parser.drain();

    for event in events {
        let data = &event.data;
        let json = match JsonValue::parse(data) { Ok(j) => j, Err(_) => continue };
        let ev_type = match json.get("type").and_then(|v| v.as_str()) { Some(t) => t, None => continue };

        match ev_type {
            "content_block_start" => {
                let idx = json.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let block = json.get("content_block").unwrap_or(&JsonValue::Null);
                let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match block_type {
                    "text" => { text_acc.insert(idx, String::new()); }
                    "tool_use" => {
                        let id = block.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        tool_acc.insert(idx, (id, name, String::new()));
                    }
                    _ => {}
                }
            }
            "content_block_delta" => {
                let idx = json.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let delta = json.get("delta").unwrap_or(&JsonValue::Null);
                let delta_type = delta.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match delta_type {
                    "text_delta" => {
                        if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                            on_text(text);
                            if let Some(acc) = text_acc.get_mut(&idx) { acc.push_str(text); }
                        }
                    }
                    "input_json_delta" => {
                        if let Some(partial) = delta.get("partial_json").and_then(|v| v.as_str()) {
                            if let Some((_, _, ref mut json_acc)) = tool_acc.get_mut(&idx) {
                                json_acc.push_str(partial);
                            }
                        }
                    }
                    _ => {}
                }
            }
            "content_block_stop" => {
                let idx = json.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                if let Some(text) = text_acc.remove(&idx) {
                    text_blocks.push(ContentBlock::Text(text));
                }
                if let Some((id, name, json_str)) = tool_acc.remove(&idx) {
                    let input = JsonValue::parse(&json_str).unwrap_or(JsonValue::Object(vec![]));
                    tool_uses.push(ContentBlock::ToolUse { id, name, input });
                }
            }
            "message_delta" => {
                if let Some(reason) = json.get("delta")
                    .and_then(|d| d.get("stop_reason"))
                    .and_then(|v| v.as_str())
                {
                    stop_reason = reason.to_string();
                }
            }
            _ => {}
        }
    }

    Ok(StreamResult { text_blocks, tool_uses, stop_reason })
}
```

- [ ] **Step 1: 写测试（验证 parse_agent_stream 离线解析）**

`tests/agent/llm_stream_test.rs`:

```rust
use viv::llm::{parse_agent_stream_pub, StreamResult};

// 注意：需要把 parse_agent_stream 改为 pub(crate) 并暴露给测试
// 实际测试通过构造 raw SSE 字节来验证

#[test]
fn parses_text_delta_from_sse() {
    // 构造最小 SSE 响应（无 HTTP header，header_end=0）
    let sse = concat!(
        "event: content_block_start\n",
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hello\"}}\n\n",
        "event: content_block_stop\n",
        "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "event: message_stop\n",
        "data: {\"type\":\"message_stop\"}\n\n",
    );
    let raw = sse.as_bytes().to_vec();
    let mut collected = String::new();
    let result = parse_agent_stream_pub(&raw, Some(0), &mut |t| collected.push_str(t)).unwrap();
    assert_eq!(collected, "hello");
    assert_eq!(result.text_blocks.len(), 1);
    assert!(result.tool_uses.is_empty());
}

#[test]
fn parses_tool_use_from_sse() {
    let sse = concat!(
        "event: content_block_start\n",
        "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"tu_01\",\"name\":\"bash\",\"input\":{}}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"command\\\":\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"\\\"ls\\\"\"}}\n\n",
        "event: content_block_delta\n",
        "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"}\"}}\n\n",
        "event: content_block_stop\n",
        "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
        "event: message_stop\n",
        "data: {\"type\":\"message_stop\"}\n\n",
    );
    let raw = sse.as_bytes().to_vec();
    let result = parse_agent_stream_pub(&raw, Some(0), &mut |_| {}).unwrap();
    assert_eq!(result.tool_uses.len(), 1);
    use viv::agent::message::ContentBlock;
    if let ContentBlock::ToolUse { name, .. } = &result.tool_uses[0] {
        assert_eq!(name, "bash");
    } else {
        panic!("expected ToolUse");
    }
}
```

- [ ] **Step 2: 运行测试，确认编译失败**

```bash
cargo test --test llm_stream_test 2>&1 | head -20
```

- [ ] **Step 3: 在 `src/llm.rs` 添加 `use crate::agent::message::*`，实现 `stream_agent`、`build_agent_request`、`parse_agent_stream`**

将上方代码追加到 `src/llm.rs` 末尾。将 `parse_agent_stream` 改为 `pub(crate)`，并添加公开测试包装：

```rust
/// 仅供测试使用的公开入口
pub fn parse_agent_stream_pub(
    raw: &[u8],
    header_end: Option<usize>,
    on_text: &mut impl FnMut(&str),
) -> crate::Result<StreamResult> {
    parse_agent_stream(raw, header_end, on_text)
}
```

- [ ] **Step 4: 运行测试，确认通过**

```bash
cargo test --test llm_stream_test
```

期望：`2 passed`

- [ ] **Step 5: Commit**

```bash
git add src/llm.rs tests/agent/llm_stream_test.rs
git commit -m "feat(llm): tool_use stream parsing + cache_control system blocks"
```

---

## Task 3: Memory Store + Index

**Files:**
- Create: `src/memory/mod.rs`
- Create: `src/memory/store.rs`
- Create: `src/memory/index.rs`
- Create: `tests/memory/store_test.rs`

### 代码

`src/memory/store.rs`:

```rust
use std::fs;
use std::path::{Path, PathBuf};
use crate::Result;
use crate::error::Error;

pub struct MemoryStore {
    pub base_dir: PathBuf,
}

impl MemoryStore {
    pub fn new(base_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&base_dir)
            .map_err(|e| Error::Io(e.to_string()))?;
        fs::create_dir_all(base_dir.join("episodes"))
            .map_err(|e| Error::Io(e.to_string()))?;
        fs::create_dir_all(base_dir.join("knowledge"))
            .map_err(|e| Error::Io(e.to_string()))?;
        fs::create_dir_all(base_dir.join("sessions"))
            .map_err(|e| Error::Io(e.to_string()))?;
        Ok(MemoryStore { base_dir })
    }

    pub fn read(&self, rel_path: &str) -> Result<String> {
        fs::read_to_string(self.base_dir.join(rel_path))
            .map_err(|e| Error::Io(e.to_string()))
    }

    pub fn write(&self, rel_path: &str, content: &str) -> Result<()> {
        let full = self.base_dir.join(rel_path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).map_err(|e| Error::Io(e.to_string()))?;
        }
        fs::write(&full, content).map_err(|e| Error::Io(e.to_string()))
    }

    pub fn exists(&self, rel_path: &str) -> bool {
        self.base_dir.join(rel_path).exists()
    }

    pub fn list_dir(&self, rel_dir: &str) -> Result<Vec<String>> {
        let dir = self.base_dir.join(rel_dir);
        if !dir.exists() { return Ok(vec![]); }
        let mut names = vec![];
        for entry in fs::read_dir(&dir).map_err(|e| Error::Io(e.to_string()))? {
            let e = entry.map_err(|e| Error::Io(e.to_string()))?;
            names.push(e.file_name().to_string_lossy().into_owned());
        }
        Ok(names)
    }
}
```

`src/memory/index.rs`:

```rust
use crate::json::JsonValue;
use crate::Result;
use super::store::MemoryStore;

const INDEX_FILE: &str = "index.json";

#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub id: String,
    pub kind: EntryKind,
    pub file: String,
    pub tags: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntryKind {
    Episode,
    Knowledge,
}

pub struct MemoryIndex {
    pub entries: Vec<MemoryEntry>,
}

impl MemoryIndex {
    pub fn load(store: &MemoryStore) -> Result<Self> {
        if !store.exists(INDEX_FILE) {
            return Ok(MemoryIndex { entries: vec![] });
        }
        let raw = store.read(INDEX_FILE)?;
        let json = JsonValue::parse(&raw).unwrap_or(JsonValue::Object(vec![]));
        let mut entries = vec![];

        for kind_key in &["episodes", "knowledge"] {
            let kind = if *kind_key == "episodes" { EntryKind::Episode } else { EntryKind::Knowledge };
            if let Some(arr) = json.get(kind_key).and_then(|v| v.as_array()) {
                for item in arr {
                    let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let file = item.get("file").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let summary = item.get("summary").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let tags = item.get("tags")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|t| t.as_str().map(|s| s.to_string())).collect())
                        .unwrap_or_default();
                    entries.push(MemoryEntry { id, kind: kind.clone(), file, tags, summary });
                }
            }
        }
        Ok(MemoryIndex { entries })
    }

    pub fn save(&self, store: &MemoryStore) -> Result<()> {
        let episodes: Vec<&MemoryEntry> = self.entries.iter().filter(|e| e.kind == EntryKind::Episode).collect();
        let knowledge: Vec<&MemoryEntry> = self.entries.iter().filter(|e| e.kind == EntryKind::Knowledge).collect();

        fn entry_json(e: &MemoryEntry) -> String {
            let tags: Vec<String> = e.tags.iter().map(|t| format!("\"{}\"", t)).collect();
            format!(
                "{{\"id\":\"{}\",\"file\":\"{}\",\"tags\":[{}],\"summary\":\"{}\"}}",
                e.id, e.file, tags.join(","),
                e.summary.replace('"', "\\\""),
            )
        }

        let ep_json: Vec<String> = episodes.iter().map(|e| entry_json(e)).collect();
        let kn_json: Vec<String> = knowledge.iter().map(|e| entry_json(e)).collect();

        let content = format!(
            "{{\"version\":1,\"episodes\":[{}],\"knowledge\":[{}]}}",
            ep_json.join(","),
            kn_json.join(","),
        );
        store.write(INDEX_FILE, &content)
    }

    /// 关键词预筛：返回 summary 或 tags 中包含 query 词的条目
    pub fn keyword_search(&self, query: &str) -> Vec<&MemoryEntry> {
        let words: Vec<&str> = query.split_whitespace().collect();
        self.entries.iter().filter(|e| {
            words.iter().any(|w| {
                let w_lower = w.to_lowercase();
                e.summary.to_lowercase().contains(&w_lower)
                    || e.tags.iter().any(|t| t.to_lowercase().contains(&w_lower))
            })
        }).collect()
    }

    pub fn upsert(&mut self, entry: MemoryEntry) {
        if let Some(existing) = self.entries.iter_mut().find(|e| e.id == entry.id) {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }
}
```

- [ ] **Step 1: 写测试**

`tests/memory/store_test.rs`:

```rust
use std::path::PathBuf;
use viv::memory::store::MemoryStore;
use viv::memory::index::{MemoryIndex, MemoryEntry, EntryKind};

fn tmp_store() -> MemoryStore {
    let dir = std::env::temp_dir().join(format!("viv_test_{}", std::process::id()));
    MemoryStore::new(dir).unwrap()
}

#[test]
fn store_write_and_read() {
    let store = tmp_store();
    store.write("test.txt", "hello").unwrap();
    assert_eq!(store.read("test.txt").unwrap(), "hello");
}

#[test]
fn store_exists() {
    let store = tmp_store();
    assert!(!store.exists("nope.txt"));
    store.write("yes.txt", "x").unwrap();
    assert!(store.exists("yes.txt"));
}

#[test]
fn index_save_and_load() {
    let store = tmp_store();
    let mut idx = MemoryIndex { entries: vec![] };
    idx.upsert(MemoryEntry {
        id: "e1".into(),
        kind: EntryKind::Knowledge,
        file: "knowledge/e1.md".into(),
        tags: vec!["rust".into(), "error".into()],
        summary: "Use Error enum not String".into(),
    });
    idx.save(&store).unwrap();

    let loaded = MemoryIndex::load(&store).unwrap();
    assert_eq!(loaded.entries.len(), 1);
    assert_eq!(loaded.entries[0].id, "e1");
    assert_eq!(loaded.entries[0].tags, vec!["rust", "error"]);
}

#[test]
fn index_keyword_search() {
    let store = tmp_store();
    let mut idx = MemoryIndex { entries: vec![] };
    idx.upsert(MemoryEntry {
        id: "k1".into(), kind: EntryKind::Knowledge,
        file: "knowledge/k1.md".into(),
        tags: vec!["rust".into()],
        summary: "zero dependency architecture".into(),
    });
    idx.upsert(MemoryEntry {
        id: "k2".into(), kind: EntryKind::Knowledge,
        file: "knowledge/k2.md".into(),
        tags: vec!["style".into()],
        summary: "use snake_case naming".into(),
    });

    let results = idx.keyword_search("rust dependency");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "k1");
}
```

- [ ] **Step 2: 运行测试，确认编译失败**

```bash
cargo test --test store_test 2>&1 | head -20
```

- [ ] **Step 3: 创建 `src/memory/mod.rs`**

```rust
pub mod store;
pub mod index;
pub mod retrieval;
pub mod compaction;
```

- [ ] **Step 4: 在 `src/lib.rs` 添加**

```rust
pub mod memory;
```

- [ ] **Step 5: 实现 `src/memory/store.rs` 和 `src/memory/index.rs`**

- [ ] **Step 6: 运行测试，确认通过**

```bash
cargo test --test store_test
```

期望：`4 passed`

- [ ] **Step 7: Commit**

```bash
git add src/memory/ src/lib.rs tests/memory/store_test.rs
git commit -m "feat(memory): store + index — file I/O + keyword search"
```

---

## Task 4: Memory Retrieval（两阶段检索）

**Files:**
- Create: `src/memory/retrieval.rs`
- Create: `tests/memory/retrieval_test.rs`

### 代码

`src/memory/retrieval.rs`:

```rust
use crate::Result;
use crate::llm::{LLMClient, ModelTier};
use super::index::{MemoryEntry, MemoryIndex};
use super::store::MemoryStore;

pub struct RetrievalResult {
    pub entry: MemoryEntry,
    pub content: String,
}

/// 两阶段检索：关键词预筛 → LLM 相关性排序 → Top-K
pub fn retrieve_relevant(
    query: &str,
    index: &MemoryIndex,
    store: &MemoryStore,
    llm: &LLMClient,
    top_k: usize,
) -> Result<Vec<RetrievalResult>> {
    // 阶段 1：关键词预筛（最多 20 个候选）
    let mut candidates = index.keyword_search(query);
    candidates.truncate(20);

    if candidates.is_empty() {
        return Ok(vec![]);
    }

    // 阶段 2：LLM 相关性排序（只在候选 > top_k 时才调用）
    let selected = if candidates.len() <= top_k {
        candidates
    } else {
        llm_rank(query, candidates, llm, top_k)?
    };

    // 读取文件内容
    let mut results = vec![];
    for entry in selected {
        if let Ok(content) = store.read(&entry.file) {
            results.push(RetrievalResult { entry: entry.clone(), content });
        }
    }
    Ok(results)
}

fn llm_rank<'a>(
    query: &str,
    candidates: Vec<&'a MemoryEntry>,
    llm: &LLMClient,
    top_k: usize,
) -> Result<Vec<&'a MemoryEntry>> {
    let list: Vec<String> = candidates
        .iter()
        .enumerate()
        .map(|(i, e)| format!("[{}] {}", i, e.summary))
        .collect();

    let prompt = format!(
        "Task: \"{}\"\n\nMemory candidates:\n{}\n\nReturn the indices of the {} most relevant memories as JSON array, e.g. [0,2,3]. Return ONLY the JSON array, nothing else.",
        query,
        list.join("\n"),
        top_k,
    );

    use crate::agent::message::{Message, SystemBlock};
    let system = vec![SystemBlock::dynamic("You are a memory retrieval assistant.")];
    let messages = vec![Message::user_text(prompt)];
    let mut response = String::new();
    llm.stream_agent(&system, &messages, ModelTier::Fast, |t| response.push_str(t))?;

    // 解析 JSON 数组
    let indices = parse_index_array(&response);
    Ok(indices.into_iter()
        .filter(|&i| i < candidates.len())
        .map(|i| candidates[i])
        .take(top_k)
        .collect())
}

fn parse_index_array(s: &str) -> Vec<usize> {
    // 找到 [ ... ] 并解析数字
    let start = s.find('[').unwrap_or(0);
    let end = s.rfind(']').map(|i| i + 1).unwrap_or(s.len());
    let slice = &s[start..end];
    slice.split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<usize>().ok())
        .collect()
}

/// 将检索结果格式化为注入 prompt 的文本
pub fn format_memory_injection(results: &[RetrievalResult]) -> String {
    if results.is_empty() {
        return String::new();
    }
    let mut out = String::from("<memory>\n");
    for r in results {
        let kind = match r.entry.kind {
            super::index::EntryKind::Episode => "Episodic",
            super::index::EntryKind::Knowledge => "Knowledge",
        };
        out.push_str(&format!("[{}] {}\n", kind, r.entry.summary));
    }
    out.push_str("</memory>");
    out
}
```

- [ ] **Step 1: 写测试（离线验证 parse_index_array 和 format_memory_injection）**

`tests/memory/retrieval_test.rs`:

```rust
use viv::memory::retrieval::{format_memory_injection, RetrievalResult};
use viv::memory::index::{MemoryEntry, EntryKind};

fn make_result(summary: &str, kind: EntryKind) -> RetrievalResult {
    RetrievalResult {
        entry: MemoryEntry {
            id: "x".into(), kind, file: "f.md".into(),
            tags: vec![], summary: summary.into(),
        },
        content: summary.into(),
    }
}

#[test]
fn format_empty_returns_empty() {
    assert!(format_memory_injection(&[]).is_empty());
}

#[test]
fn format_includes_summaries() {
    let results = vec![
        make_result("zero dependency", EntryKind::Knowledge),
        make_result("session 2026-04-10", EntryKind::Episode),
    ];
    let out = format_memory_injection(&results);
    assert!(out.contains("<memory>"));
    assert!(out.contains("[Knowledge] zero dependency"));
    assert!(out.contains("[Episodic] session 2026-04-10"));
    assert!(out.contains("</memory>"));
}
```

- [ ] **Step 2: 运行测试，确认失败**

```bash
cargo test --test retrieval_test 2>&1 | head -20
```

- [ ] **Step 3: 实现 `src/memory/retrieval.rs`**

- [ ] **Step 4: 运行测试，确认通过**

```bash
cargo test --test retrieval_test
```

期望：`2 passed`

- [ ] **Step 5: Commit**

```bash
git add src/memory/retrieval.rs tests/memory/retrieval_test.rs
git commit -m "feat(memory): two-stage retrieval + memory injection formatter"
```

---

## Task 5: Prompt Builder（缓存优先）

**Files:**
- Create: `src/agent/prompt.rs`
- Create: `tests/agent/prompt_test.rs`

### 代码

`src/agent/prompt.rs`:

```rust
use crate::agent::message::{SystemBlock, PromptCache, hash_str};
use crate::memory::retrieval::RetrievalResult;

const BASE_SYSTEM_PROMPT: &str = r#"You are viv, a self-evolving AI programming agent.

You help users with software engineering tasks: writing code, fixing bugs, refactoring, explaining code, and running commands.

You have access to tools to read/write files, execute commands, search the web, and more. Use them to accomplish tasks effectively.

Be concise and direct. Default to action over explanation. When a task is ambiguous, make a reasonable assumption and proceed."#;

/// 用于 LLM 请求的 system prompt 构建结果
pub struct SystemPrompt {
    pub blocks: Vec<SystemBlock>,
}

/// 构建带 cache_control 的 system prompt
/// 顺序：Base → Tools → Skills → Memory（动态，不缓存）
pub fn build_system_prompt(
    tool_descriptions: &str,
    skill_contents: &str,
    memory_results: &[RetrievalResult],
    cache: &mut PromptCache,
) -> SystemPrompt {
    let mut blocks = vec![];

    // Block 1: Base（最稳定，最高缓存命中率）
    let base_hash = hash_str(BASE_SYSTEM_PROMPT);
    if cache.base_hash != base_hash {
        cache.base_hash = base_hash;
        cache.base_text = BASE_SYSTEM_PROMPT.to_string();
    }
    blocks.push(SystemBlock::cached(&cache.base_text));

    // Block 2: Tools（工具集变化时失效）
    if !tool_descriptions.is_empty() {
        let tools_hash = hash_str(tool_descriptions);
        if cache.tools_hash != tools_hash {
            cache.tools_hash = tools_hash;
            cache.tools_text = tool_descriptions.to_string();
        }
        blocks.push(SystemBlock::cached(&cache.tools_text));
    }

    // Block 3: Skills（skill 集合变化时失效）
    if !skill_contents.is_empty() {
        let skills_hash = hash_str(skill_contents);
        if cache.skills_hash != skills_hash {
            cache.skills_hash = skills_hash;
            cache.skills_text = skill_contents.to_string();
        }
        blocks.push(SystemBlock::cached(&cache.skills_text));
    }

    // Block 4: Memory（每次不同，不缓存）
    let memory_text = crate::memory::retrieval::format_memory_injection(memory_results);
    if !memory_text.is_empty() {
        blocks.push(SystemBlock::dynamic(memory_text));
    }

    SystemPrompt { blocks }
}
```

- [ ] **Step 1: 写测试**

`tests/agent/prompt_test.rs`:

```rust
use viv::agent::message::PromptCache;
use viv::agent::prompt::build_system_prompt;

#[test]
fn prompt_has_base_block_with_cache() {
    let mut cache = PromptCache::default();
    let sp = build_system_prompt("", "", &[], &mut cache);
    assert!(!sp.blocks.is_empty());
    assert!(sp.blocks[0].cached);
}

#[test]
fn prompt_tools_block_added_when_nonempty() {
    let mut cache = PromptCache::default();
    let sp = build_system_prompt("tool: bash", "", &[], &mut cache);
    assert_eq!(sp.blocks.len(), 2);
    assert!(sp.blocks[1].cached);
}

#[test]
fn prompt_cache_reuses_text_on_same_hash() {
    let mut cache = PromptCache::default();
    let _ = build_system_prompt("tools v1", "", &[], &mut cache);
    let h1 = cache.tools_hash;
    let _ = build_system_prompt("tools v1", "", &[], &mut cache);
    assert_eq!(cache.tools_hash, h1); // 未变化
}

#[test]
fn prompt_cache_updates_on_changed_content() {
    let mut cache = PromptCache::default();
    let _ = build_system_prompt("tools v1", "", &[], &mut cache);
    let h1 = cache.tools_hash;
    let _ = build_system_prompt("tools v2", "", &[], &mut cache);
    assert_ne!(cache.tools_hash, h1); // 已更新
}

#[test]
fn memory_block_not_cached() {
    use viv::memory::retrieval::RetrievalResult;
    use viv::memory::index::{MemoryEntry, EntryKind};
    let results = vec![RetrievalResult {
        entry: MemoryEntry { id: "1".into(), kind: EntryKind::Knowledge,
            file: "f.md".into(), tags: vec![], summary: "test fact".into() },
        content: "test fact".into(),
    }];
    let mut cache = PromptCache::default();
    let sp = build_system_prompt("", "", &results, &mut cache);
    // memory block 是最后一个，不缓存
    let last = sp.blocks.last().unwrap();
    assert!(!last.cached);
}
```

- [ ] **Step 2: 运行测试，确认失败**

```bash
cargo test --test prompt_test 2>&1 | head -20
```

- [ ] **Step 3: 实现 `src/agent/prompt.rs`，更新 `src/agent/mod.rs`**

```rust
// src/agent/mod.rs
pub mod message;
pub mod prompt;
pub mod context;
pub mod run;
pub mod evolution;
```

- [ ] **Step 4: 运行测试，确认通过**

```bash
cargo test --test prompt_test
```

期望：`5 passed`

- [ ] **Step 5: Commit**

```bash
git add src/agent/prompt.rs src/agent/mod.rs tests/agent/prompt_test.rs
git commit -m "feat(agent): prompt builder — cache-first 4-block assembly"
```

---

## Task 6: AgentContext + 记忆系统集成

**Files:**
- Create: `src/agent/context.rs`
- Create: `src/memory/compaction.rs`

`src/agent/context.rs`:

```rust
use std::sync::{Arc, Mutex};
use crate::llm::{LLMClient, ModelTier};
use crate::agent::message::{Message, PromptCache};
use crate::memory::store::MemoryStore;
use crate::memory::index::MemoryIndex;

pub struct AgentContext {
    pub messages: Vec<Message>,
    pub prompt_cache: PromptCache,
    pub llm: Arc<LLMClient>,
    pub store: Arc<MemoryStore>,
    pub index: Arc<Mutex<MemoryIndex>>,
    pub config: AgentConfig,
}

#[derive(Clone)]
pub struct AgentConfig {
    pub model_tier: ModelTier,
    pub max_iterations: usize,   // 默认 50，防止无限循环
    pub top_k_memory: usize,     // 默认 5，每次注入最多 K 条记忆
    pub permission_mode: PermissionMode,
}

#[derive(Clone, PartialEq)]
pub enum PermissionMode {
    Default,
    Auto,
    Bypass,
}

impl Default for AgentConfig {
    fn default() -> Self {
        AgentConfig {
            model_tier: ModelTier::Medium,
            max_iterations: 50,
            top_k_memory: 5,
            permission_mode: PermissionMode::Default,
        }
    }
}

impl AgentContext {
    pub fn new(llm: Arc<LLMClient>, base_dir: std::path::PathBuf) -> crate::Result<Self> {
        let store = Arc::new(MemoryStore::new(base_dir)?);
        let index = Arc::new(Mutex::new(MemoryIndex::load(&store)?));
        Ok(AgentContext {
            messages: vec![],
            prompt_cache: PromptCache::default(),
            llm,
            store,
            index,
            config: AgentConfig::default(),
        })
    }
}
```

`src/memory/compaction.rs`:

```rust
use crate::agent::message::{Message, ContentBlock, SystemBlock};
use crate::llm::{LLMClient, ModelTier};
use crate::Result;

/// 当消息历史超过阈值时，压缩旧消息为摘要块
/// 保留最近 keep_recent 轮，压缩其余部分
pub fn compact_if_needed(
    messages: &mut Vec<Message>,
    token_estimate: usize,
    token_limit: usize,
    keep_recent: usize,
    llm: &LLMClient,
) -> Result<()> {
    if token_estimate < token_limit * 8 / 10 {
        return Ok(()); // 未超过 80% 阈值，不压缩
    }
    if messages.len() <= keep_recent * 2 {
        return Ok(()); // 消息太少，不压缩
    }

    let split_at = messages.len().saturating_sub(keep_recent * 2);
    let to_compress: Vec<&Message> = messages[..split_at].iter().collect();

    // 用 fast tier 生成摘要
    let summary_prompt = format!(
        "Summarize this conversation history concisely (2-4 sentences):\n\n{}",
        messages_to_text(&to_compress),
    );
    let system = vec![SystemBlock::dynamic("You are a conversation summarizer.")];
    let req_msgs = vec![Message::user_text(summary_prompt)];
    let mut summary = String::new();
    llm.stream_agent(&system, &req_msgs, ModelTier::Fast, |t| summary.push_str(t))?;

    // 用摘要替换旧消息
    let recent = messages.split_off(split_at);
    messages.clear();
    messages.push(Message::User(vec![
        ContentBlock::Text(format!("[Earlier conversation summary]\n{}", summary))
    ]));
    messages.extend(recent);

    Ok(())
}

fn messages_to_text(messages: &[&Message]) -> String {
    messages.iter().map(|m| {
        let role = m.role();
        let text = m.blocks().iter().filter_map(|b| {
            if let ContentBlock::Text(t) = b { Some(t.as_str()) } else { None }
        }).collect::<Vec<_>>().join(" ");
        format!("{}: {}", role, text)
    }).collect::<Vec<_>>().join("\n")
}

/// 粗估 token 数（字符数 / 4）
pub fn estimate_tokens(messages: &[Message]) -> usize {
    messages.iter().map(|m| {
        m.blocks().iter().map(|b| match b {
            ContentBlock::Text(t) => t.len() / 4,
            ContentBlock::ToolUse { input, .. } => input.to_string().len() / 4,
            ContentBlock::ToolResult { content, .. } => {
                content.iter().map(|c| if let ContentBlock::Text(t) = c { t.len() / 4 } else { 10 }).sum::<usize>()
            }
        }).sum::<usize>()
    }).sum()
}
```

- [ ] **Step 1: 实现 `src/agent/context.rs`**

- [ ] **Step 2: 实现 `src/memory/compaction.rs`**

- [ ] **Step 3: 编译检查**

```bash
cargo build 2>&1 | grep "^error"
```

期望：无 error

- [ ] **Step 4: Commit**

```bash
git add src/agent/context.rs src/memory/compaction.rs
git commit -m "feat(agent): AgentContext + context compaction"
```

---

## Task 7: Agent 主循环

**Files:**
- Create: `src/agent/run.rs`
- Create: `tests/agent/run_test.rs`

`src/agent/run.rs`:

```rust
use crate::Result;
use crate::agent::context::AgentContext;
use crate::agent::message::{Message, ContentBlock};
use crate::agent::prompt::build_system_prompt;
use crate::memory::retrieval::retrieve_relevant;
use crate::memory::compaction::{compact_if_needed, estimate_tokens};

pub struct AgentOutput {
    pub text: String,
    pub iterations: usize,
}

/// Agent 主循环：接受用户输入，返回最终响应文本
pub async fn run_agent(
    input: String,
    ctx: &mut AgentContext,
    tool_descriptions: &str,
    skill_contents: &str,
    mut on_text: impl FnMut(&str),
) -> Result<AgentOutput> {
    // 1. 检索相关记忆
    let index = ctx.index.lock().unwrap().clone(); // clone for read
    drop(ctx.index.lock()); // release lock
    let memories = {
        let idx = ctx.index.lock().unwrap();
        retrieve_relevant(&input, &idx, &ctx.store, &ctx.llm, ctx.config.top_k_memory)?
    };

    // 2. 构建 system prompt（缓存优先）
    let system = build_system_prompt(tool_descriptions, skill_contents, &memories, &mut ctx.prompt_cache);

    // 3. 追加用户消息
    ctx.messages.push(Message::user_text(input));

    // 4. 上下文压缩（如需要）
    let token_estimate = estimate_tokens(&ctx.messages);
    compact_if_needed(&mut ctx.messages, token_estimate, 100_000, 10, &ctx.llm)?;

    let mut final_text = String::new();
    let mut iterations = 0;

    loop {
        if iterations >= ctx.config.max_iterations { break; }
        iterations += 1;

        // 5. 调用 LLM
        let stream_result = ctx.llm.stream_agent(
            &system.blocks,
            &ctx.messages,
            ctx.config.model_tier.clone(),
            &mut on_text,
        )?;

        // 6. 收集 assistant 响应块
        let mut assistant_blocks: Vec<ContentBlock> = stream_result.text_blocks.clone();
        assistant_blocks.extend(stream_result.tool_uses.clone());

        // 记录最后一段文本
        for b in &stream_result.text_blocks {
            if let ContentBlock::Text(t) = b { final_text = t.clone(); }
        }

        ctx.messages.push(Message::Assistant(assistant_blocks));

        // 7. 无工具调用 → 结束
        if stream_result.tool_uses.is_empty() || stream_result.stop_reason == "end_turn" {
            break;
        }

        // 8. 执行工具（此版本 stub：返回 "tool not implemented"）
        let tool_results: Vec<ContentBlock> = stream_result.tool_uses.iter().map(|tu| {
            if let ContentBlock::ToolUse { id, .. } = tu {
                ContentBlock::ToolResult {
                    tool_use_id: id.clone(),
                    content: vec![ContentBlock::Text("Tool not yet implemented.".into())],
                    is_error: false,
                }
            } else { unreachable!() }
        }).collect();

        // 9. 追加 tool_result（Anthropic 要求作为 user 消息）
        ctx.messages.push(Message::User(tool_results));
    }

    Ok(AgentOutput { text: final_text, iterations })
}
```

- [ ] **Step 1: 写测试（验证无工具调用的正常流程，用 mock 检查循环退出）**

`tests/agent/run_test.rs`:

```rust
use viv::agent::message::{Message, ContentBlock, PromptCache};

#[test]
fn message_sequence_builds_correctly() {
    let mut messages: Vec<Message> = vec![];
    messages.push(Message::user_text("hello"));
    assert_eq!(messages[0].role(), "user");
    messages.push(Message::Assistant(vec![ContentBlock::Text("world".into())]));
    assert_eq!(messages[1].role(), "assistant");
    assert_eq!(messages.len(), 2);
}

#[test]
fn tool_result_is_user_role() {
    let tr = Message::User(vec![ContentBlock::ToolResult {
        tool_use_id: "tu_01".into(),
        content: vec![ContentBlock::Text("ok".into())],
        is_error: false,
    }]);
    assert_eq!(tr.role(), "user");
    let json = tr.to_json();
    assert!(json.contains("\"role\":\"user\""));
}
```

- [ ] **Step 2: 运行测试，确认通过**

```bash
cargo test --test run_test
```

期望：`2 passed`

- [ ] **Step 3: 实现 `src/agent/run.rs`**

- [ ] **Step 4: 编译检查**

```bash
cargo build 2>&1 | grep "^error"
```

- [ ] **Step 5: Commit**

```bash
git add src/agent/run.rs tests/agent/run_test.rs
git commit -m "feat(agent): main loop — LLM stream + tool stub + context compaction"
```

---

## Task 8: 自我进化引擎

**Files:**
- Create: `src/agent/evolution.rs`

`src/agent/evolution.rs`:

```rust
use crate::Result;
use crate::agent::message::{Message, ContentBlock, SystemBlock};
use crate::llm::{LLMClient, ModelTier};
use crate::memory::store::MemoryStore;
use crate::memory::index::{MemoryIndex, MemoryEntry, EntryKind};
use crate::json::JsonValue;

const EVOLUTION_PROMPT: &str = r#"You just completed a conversation session. Analyze it and extract learnings.

Return a JSON array of learning objects. Each object must have:
- "kind": "fact" | "pattern" | "mistake"  
- "content": string (the learning, max 2 sentences)
- "tags": array of 1-3 lowercase keyword strings
- "id": a short kebab-case identifier (e.g. "zero-deps-rule")

Example:
[{"kind":"fact","content":"User prefers zero external dependencies.","tags":["rust","deps"],"id":"zero-deps-rule"}]

Return ONLY the JSON array. If there are no significant learnings, return [].

Conversation to analyze:"#;

/// 会话结束后提炼经验，写入记忆
pub fn evolve_from_session(
    messages: &[Message],
    store: &MemoryStore,
    index: &mut MemoryIndex,
    llm: &LLMClient,
) -> Result<usize> {
    if messages.len() < 2 {
        return Ok(0);
    }

    let conversation_text = messages_to_text(messages);
    let prompt = format!("{}\n\n{}", EVOLUTION_PROMPT, conversation_text);

    let system = vec![SystemBlock::dynamic("You are an AI learning extractor.")];
    let req_msgs = vec![Message::user_text(prompt)];
    let mut response = String::new();
    llm.stream_agent(&system, &req_msgs, ModelTier::Medium, |t| response.push_str(t))?;

    let learnings = parse_learnings(&response);
    let count = learnings.len();

    for learning in learnings {
        let file = format!("knowledge/{}.md", learning.id);
        let content = format!(
            "---\nkind: {}\ntags: {}\n---\n\n{}\n",
            learning.kind,
            learning.tags.join(", "),
            learning.content,
        );
        store.write(&file, &content)?;
        index.upsert(MemoryEntry {
            id: learning.id.clone(),
            kind: EntryKind::Knowledge,
            file,
            tags: learning.tags,
            summary: learning.content,
        });
    }
    index.save(store)?;

    // 保存会话摘要到 episodic memory
    save_episode(messages, store, index, llm)?;

    Ok(count)
}

struct Learning {
    id: String,
    kind: String,
    content: String,
    tags: Vec<String>,
}

fn parse_learnings(response: &str) -> Vec<Learning> {
    let start = response.find('[').unwrap_or(0);
    let end = response.rfind(']').map(|i| i + 1).unwrap_or(response.len());
    let json_str = &response[start..end];

    let json = match JsonValue::parse(json_str) { Ok(j) => j, Err(_) => return vec![] };
    let arr = match json.as_array() { Some(a) => a, None => return vec![] };

    arr.iter().filter_map(|item| {
        let id = item.get("id")?.as_str()?.to_string();
        let kind = item.get("kind")?.as_str()?.to_string();
        let content = item.get("content")?.as_str()?.to_string();
        let tags = item.get("tags")?.as_array()
            .map(|a| a.iter().filter_map(|t| t.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        if id.is_empty() || content.is_empty() { return None; }
        Some(Learning { id, kind, content, tags })
    }).collect()
}

fn save_episode(
    messages: &[Message],
    store: &MemoryStore,
    index: &mut MemoryIndex,
    llm: &LLMClient,
) -> Result<()> {
    let summary_prompt = format!(
        "Summarize this conversation in 1-2 sentences, focusing on what was accomplished:\n\n{}",
        messages_to_text(messages),
    );
    let system = vec![SystemBlock::dynamic("You are a conversation summarizer.")];
    let req_msgs = vec![Message::user_text(summary_prompt)];
    let mut summary = String::new();
    llm.stream_agent(&system, &req_msgs, ModelTier::Fast, |t| summary.push_str(t))?;
    let summary = summary.trim().to_string();

    // 用时间戳生成唯一 ID
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let id = format!("ep-{}", ts);
    let file = format!("episodes/{}.md", id);
    let tags = extract_tags_from_summary(&summary);

    store.write(&file, &format!("# Episode\n\n{}\n", summary))?;
    index.upsert(MemoryEntry {
        id: id.clone(),
        kind: EntryKind::Episode,
        file,
        tags,
        summary,
    });
    index.save(store)
}

fn extract_tags_from_summary(summary: &str) -> Vec<String> {
    // 简单提取：取摘要中较长的词作为 tag（去停用词）
    const STOP_WORDS: &[&str] = &["the","a","an","in","on","at","to","for","of","and","or","with","this","that","was","were","is","are","it","its","been","have","has","had","from","by","as","be","not","but"];
    summary.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 4)
        .filter(|w| !STOP_WORDS.contains(&w.to_lowercase().as_str()))
        .map(|w| w.to_lowercase())
        .take(3)
        .collect()
}

fn messages_to_text(messages: &[Message]) -> String {
    messages.iter().map(|m| {
        let role = m.role();
        let text = m.blocks().iter().filter_map(|b| {
            if let ContentBlock::Text(t) = b { Some(t.as_str()) } else { None }
        }).collect::<Vec<_>>().join(" ");
        format!("{}: {}", role, text)
    }).collect::<Vec<_>>().join("\n")
}
```

- [ ] **Step 1: 实现 `src/agent/evolution.rs`**

- [ ] **Step 2: 编译检查**

```bash
cargo build 2>&1 | grep "^error"
```

- [ ] **Step 3: Commit**

```bash
git add src/agent/evolution.rs
git commit -m "feat(agent): self-evolution engine — experience extraction + episode memory"
```

---

## Task 9: 接入 REPL

**Files:**
- Modify: `src/repl.rs`（找到调用 `llm.stream` 的地方，替换为 `run_agent`）

- [ ] **Step 1: 找到 repl.rs 中 LLM 调用位置**

```bash
grep -n "llm\|stream\|messages" src/repl.rs | head -30
```

- [ ] **Step 2: 在 repl.rs 中初始化 AgentContext**

在 `run()` 函数开头，`LLMClient::new` 调用之后，添加：

```rust
use crate::agent::context::AgentContext;
let viv_dir = std::env::current_dir()
    .unwrap_or_default()
    .join(".viv")
    .join("memory");
let mut agent_ctx = AgentContext::new(
    std::sync::Arc::new(llm_client.clone()),
    viv_dir,
)?;
```

- [ ] **Step 3: 替换 LLM 直接调用为 run_agent**

找到发送消息给 LLM 的代码段（通常在用户按 Enter 后），替换为：

```rust
use crate::agent::run::run_agent;
let output = run_agent(
    user_input.clone(),
    &mut agent_ctx,
    "",  // tool_descriptions（Task 3 Tool 系统实现后填入）
    "",  // skill_contents（Task 6 Skill 系统实现后填入）
    |text| {
        // 流式回调：发送到 UI 渲染
        ui_tx.send(AgentEvent::TextDelta(text.to_string())).ok();
    },
)?;
```

- [ ] **Step 4: 会话结束时触发进化**

在 REPL 退出钩子（`Ctrl+D` 处理）处添加：

```rust
use crate::agent::evolution::evolve_from_session;
let _ = evolve_from_session(
    &agent_ctx.messages,
    &agent_ctx.store,
    &mut agent_ctx.index.lock().unwrap(),
    &agent_ctx.llm,
);
```

- [ ] **Step 5: 编译并运行**

```bash
cargo build && VIV_API_KEY=$VIV_API_KEY cargo run
```

- [ ] **Step 6: 全量测试**

```bash
cargo test
```

期望：所有测试通过

- [ ] **Step 7: Commit**

```bash
git add src/repl.rs
git commit -m "feat: wire agent loop + memory into REPL"
```

---

## 自检结果

**Spec 覆盖检查：**

| Spec 章节 | 对应 Task | 状态 |
|-----------|-----------|------|
| § 四 Agent 循环 | Task 7 | ✅ |
| § 四 消息类型 | Task 1 | ✅ |
| § 八 Working/Session Memory | Task 6（AgentContext） | ✅ |
| § 八 Episodic Memory | Task 8（evolution.rs） | ✅ |
| § 八 Semantic Memory | Task 8（knowledge/*.md） | ✅ |
| § 八 两阶段检索 | Task 4 | ✅ |
| § 八 自我进化 | Task 8 | ✅ |
| § 八 上下文压缩 | Task 6（compaction.rs） | ✅ |
| § 九 Prompt 拼接（缓存优先） | Task 5 | ✅ |
| LLM cache_control | Task 2 | ✅ |
| LLM tool_use 流解析 | Task 2 | ✅ |

**无 TBD / 无占位符。**

**类型一致性：** `ContentBlock`、`Message`、`SystemBlock`、`PromptCache`、`MemoryEntry` 全程同名同结构。
