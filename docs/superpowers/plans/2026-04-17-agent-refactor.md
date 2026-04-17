# Agent 重构实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将分散的 `AgentContext` + `run_agent()` + `repl.rs` 整合为统一的 `Agent` struct，通过双线程 + channel 实现 UI 层与 Agent 层彻底分离。

**Architecture:** Agent 持有 `Receiver<AgentEvent>` 和 `Sender<AgentMessage>`，在独立线程无限循环处理事件；TerminalUI 持有镜像 channel，在主线程负责所有渲染和输入；两者仅通过 channel 通信。

**Tech Stack:** Rust std（`std::sync::mpsc`，`std::thread`），无第三方依赖。

---

## 文件结构变化

| 操作 | 路径 | 职责 |
|------|------|------|
| 新建 | `src/bus/mod.rs` | `AgentEvent`、`AgentMessage` 枚举定义 |
| 新建 | `src/bus/terminal.rs` | `TerminalUI` struct（从 `repl.rs` 迁移重构） |
| 新建 | `src/agent/agent.rs` | `Agent` struct、`AgentConfig`、`PermissionMode`（合并 `context.rs` + `run.rs`） |
| 修改 | `src/permissions/manager.rs` | 移除 `ask_fn` 回调，改为 `is_allowed()` / `grant()` |
| 修改 | `src/main.rs` | 建 channel → spawn Agent 线程 → TerminalUI 主线程 |
| 修改 | `src/lib.rs` | 添加 `pub mod bus;`，删除 `pub mod repl;` |
| 修改 | `src/agent/mod.rs` | 添加 `pub mod agent;`，删除 `pub mod run;` / `pub mod context;` |
| 修改 | `tests/permissions/manager_test.rs` | 更新为新 API |
| 新建 | `tests/bus/mod.rs` | bus 模块测试入口 |
| 新建 | `tests/bus/bus_test.rs` | AgentEvent / AgentMessage 基础测试 |
| 删除 | `src/agent/run.rs` | 逻辑迁入 `agent.rs` |
| 删除 | `src/agent/context.rs` | 字段迁入 `Agent` struct |
| 删除 | `src/repl.rs` | 迁入 `bus/terminal.rs` |
| 删除 | `tests/agent/run_test.rs` | 测试已删除的 `run_agent()` |
| 删除 | `tests/repl_test.rs` | 测试已删除的 `repl::run()` |

---

## Task 1: 定义通信协议 `src/bus/mod.rs`

**Files:**
- Create: `src/bus/mod.rs`
- Create: `tests/bus/mod.rs`
- Create: `tests/bus/bus_test.rs`

- [ ] **Step 1: 写失败测试**

创建 `tests/bus/mod.rs`：
```rust
mod bus_test;
```

创建 `tests/bus/bus_test.rs`：
```rust
use viv::bus::{AgentEvent, AgentMessage};

#[test]
fn agent_event_input_holds_string() {
    let event = AgentEvent::Input("hello".to_string());
    match event {
        AgentEvent::Input(s) => assert_eq!(s, "hello"),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn agent_message_text_chunk_holds_string() {
    let msg = AgentMessage::TextChunk("world".to_string());
    match msg {
        AgentMessage::TextChunk(s) => assert_eq!(s, "world"),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn agent_message_permission_request_holds_fields() {
    let msg = AgentMessage::PermissionRequest {
        tool: "bash".to_string(),
        input: "ls -la".to_string(),
    };
    match msg {
        AgentMessage::PermissionRequest { tool, input } => {
            assert_eq!(tool, "bash");
            assert_eq!(input, "ls -la");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn channel_sends_events_between_threads() {
    use std::sync::mpsc::channel;
    let (tx, rx) = channel::<AgentEvent>();
    tx.send(AgentEvent::Input("test".to_string())).unwrap();
    match rx.recv().unwrap() {
        AgentEvent::Input(s) => assert_eq!(s, "test"),
        _ => panic!("wrong event"),
    }
}
```

- [ ] **Step 2: 运行测试，确认失败**

```bash
cd /data/dlab/viv && cargo test bus 2>&1 | head -20
```
Expected: 编译错误 `unresolved import viv::bus`

- [ ] **Step 3: 实现 `src/bus/mod.rs`**

```rust
pub mod terminal;

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

- [ ] **Step 4: 在 `src/lib.rs` 中添加 `pub mod bus;`**

在 `src/lib.rs` 顶部添加：
```rust
pub mod bus;
```

- [ ] **Step 5: 创建占位 `src/bus/terminal.rs`**

```rust
// 将在 Task 4 中实现
```

- [ ] **Step 6: 创建测试入口文件 `tests/bus_tests.rs`**

项目测试结构：每个模块有一个顶层 `tests/xxx_tests.rs`（内容为 `mod xxx;`）对应 `tests/xxx/mod.rs`。

创建 `tests/bus_tests.rs`：
```rust
mod bus;
```

这样 `cargo test` 会自动识别 `tests/bus/` 目录。

- [ ] **Step 7: 运行测试，确认通过**

```bash
cd /data/dlab/viv && cargo test bus
```
Expected: 4 tests pass

- [ ] **Step 8: Commit**

```bash
cd /data/dlab/viv && git add src/bus/ src/lib.rs tests/bus/ && git commit -m "feat(bus): define AgentEvent and AgentMessage protocol"
```

---

## Task 2: 重构 `PermissionManager`

**Files:**
- Modify: `src/permissions/manager.rs`
- Modify: `tests/permissions/manager_test.rs`

- [ ] **Step 1: 写新 API 的失败测试**

将 `tests/permissions/manager_test.rs` 替换为：
```rust
use viv::permissions::PermissionManager;

#[test]
fn new_manager_allows_nothing() {
    let pm = PermissionManager::default();
    assert!(!pm.is_allowed("bash"));
    assert!(!pm.is_allowed("write"));
}

#[test]
fn grant_makes_tool_allowed() {
    let mut pm = PermissionManager::default();
    assert!(!pm.is_allowed("bash"));
    pm.grant("bash");
    assert!(pm.is_allowed("bash"));
}

#[test]
fn grant_is_tool_specific() {
    let mut pm = PermissionManager::default();
    pm.grant("bash");
    assert!(pm.is_allowed("bash"));
    assert!(!pm.is_allowed("write"));
}

#[test]
fn grant_is_idempotent() {
    let mut pm = PermissionManager::default();
    pm.grant("bash");
    pm.grant("bash");
    assert!(pm.is_allowed("bash"));
}
```

- [ ] **Step 2: 运行，确认失败**

```bash
cd /data/dlab/viv && cargo test permissions 2>&1 | head -30
```
Expected: 编译错误（`is_allowed`、`grant` 不存在）

- [ ] **Step 3: 实现新 `src/permissions/manager.rs`**

```rust
use std::collections::HashSet;

pub struct PermissionManager {
    session_allowed: HashSet<String>,
}

impl Default for PermissionManager {
    fn default() -> Self {
        PermissionManager { session_allowed: HashSet::new() }
    }
}

impl PermissionManager {
    /// 检查工具是否已在本 session 被授权
    pub fn is_allowed(&self, tool_name: &str) -> bool {
        self.session_allowed.contains(tool_name)
    }

    /// 将工具标记为本 session 已授权
    pub fn grant(&mut self, tool_name: &str) {
        self.session_allowed.insert(tool_name.to_string());
    }
}
```

- [ ] **Step 4: 运行，确认通过**

```bash
cd /data/dlab/viv && cargo test permissions
```
Expected: 4 tests pass

- [ ] **Step 5: Commit**

```bash
cd /data/dlab/viv && git add src/permissions/manager.rs tests/permissions/manager_test.rs && git commit -m "refactor(permissions): replace ask_fn callback with is_allowed/grant API"
```

---

## Task 3: 创建统一 `Agent` struct

**Files:**
- Create: `src/agent/agent.rs`
- Modify: `src/agent/mod.rs`

- [ ] **Step 1: 在 `src/agent/mod.rs` 添加 `pub mod agent;`**

将 `src/agent/mod.rs` 替换为：
```rust
pub mod agent;
pub mod context;  // 暂时保留，Task 6 删除
pub mod evolution;
pub mod message;
pub mod prompt;
pub mod run;      // 暂时保留，Task 6 删除
```

- [ ] **Step 2: 实现 `src/agent/agent.rs`**

```rust
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use crate::Result;
use crate::bus::{AgentEvent, AgentMessage};
use crate::agent::message::{Message, ContentBlock, PromptCache};
use crate::agent::prompt::{build_system_prompt, SystemPrompt};
use crate::agent::evolution::evolve_from_session;
use crate::core::json::JsonValue;
use crate::llm::{LLMClient, LLMConfig, ModelTier};
use crate::memory::store::MemoryStore;
use crate::memory::index::MemoryIndex;
use crate::memory::retrieval::retrieve_relevant;
use crate::memory::compaction::{compact_if_needed, estimate_tokens};
use crate::tools::{ToolRegistry, PermissionLevel};
use crate::permissions::PermissionManager;

// ── PermissionMode ────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
pub enum PermissionMode {
    Default,  // 未授权工具询问用户
    Auto,     // 自动授权所有工具
    Bypass,   // 完全跳过权限检查
}

// ── AgentConfig ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AgentConfig {
    pub model_tier: ModelTier,
    pub max_iterations: usize,
    pub top_k_memory: usize,
    pub permission_mode: PermissionMode,
    pub memory_dir: std::path::PathBuf,
}

impl Default for AgentConfig {
    fn default() -> Self {
        AgentConfig {
            model_tier: ModelTier::Medium,
            max_iterations: 50,
            top_k_memory: 5,
            permission_mode: PermissionMode::Default,
            memory_dir: std::path::PathBuf::from(".viv/memory"),
        }
    }
}

// ── Agent ─────────────────────────────────────────────────────────────────────

pub struct Agent {
    messages: Vec<Message>,
    prompt_cache: PromptCache,
    llm: Arc<LLMClient>,
    store: Arc<MemoryStore>,
    index: Arc<Mutex<MemoryIndex>>,
    tools: ToolRegistry,
    permissions: PermissionManager,
    config: AgentConfig,
    input_tokens: u64,
    output_tokens: u64,
    event_rx: Receiver<AgentEvent>,
    msg_tx: Sender<AgentMessage>,
}

impl Agent {
    pub fn new(
        config: AgentConfig,
        event_rx: Receiver<AgentEvent>,
        msg_tx: Sender<AgentMessage>,
    ) -> Result<Self> {
        let llm_config = LLMConfig::from_env()?;
        let model_name = llm_config.model(config.model_tier.clone()).to_string();
        let llm = Arc::new(LLMClient::new(llm_config));
        let store = Arc::new(MemoryStore::new(config.memory_dir.clone())?);
        let index = Arc::new(Mutex::new(MemoryIndex::load(&store)?));
        let tools = ToolRegistry::default_tools(Arc::clone(&llm));

        let _ = msg_tx.send(AgentMessage::Ready { model: model_name });

        Ok(Agent {
            messages: vec![],
            prompt_cache: PromptCache::default(),
            llm,
            store,
            index,
            tools,
            permissions: PermissionManager::default(),
            config,
            input_tokens: 0,
            output_tokens: 0,
            event_rx,
            msg_tx,
        })
    }

    /// 无限 loop：从 event_rx 读取事件并处理，直到收到 Quit 或 channel 关闭。
    pub fn run(mut self) -> Result<()> {
        loop {
            match self.event_rx.recv() {
                Ok(AgentEvent::Input(text)) => {
                    if text.trim() == "/exit" {
                        self.evolve()?;
                        let _ = self.msg_tx.send(AgentMessage::Evolved);
                        break;
                    }
                    if let Err(e) = self.handle_input(text) {
                        let _ = self.msg_tx.send(AgentMessage::Error(e.to_string()));
                        let _ = self.msg_tx.send(AgentMessage::Done);
                    }
                }
                Ok(AgentEvent::Quit) => {
                    self.evolve()?;
                    let _ = self.msg_tx.send(AgentMessage::Evolved);
                    break;
                }
                Ok(AgentEvent::Interrupt) | Ok(AgentEvent::PermissionResponse(_)) => {
                    // 空闲时收到，忽略
                }
                Err(_) => break, // channel 关闭
            }
        }
        Ok(())
    }

    fn handle_input(&mut self, text: String) -> Result<()> {
        let _ = self.msg_tx.send(AgentMessage::Thinking);

        // 1. 检索记忆
        let memories = {
            let idx = self.index.lock().unwrap();
            let results = retrieve_relevant(
                &text, &idx, &self.store, &self.llm, self.config.top_k_memory,
            );
            drop(idx);
            match results {
                Ok(m) => {
                    let _ = self.msg_tx.send(AgentMessage::Status(
                        format!("检索记忆({} 条)…", m.len()),
                    ));
                    m
                }
                Err(_) => vec![],
            }
        };

        // 2. 构建 system prompt（cache-first）
        let system = build_system_prompt("", "", &memories, &mut self.prompt_cache);

        // 3. 追加用户消息
        self.messages.push(Message::user_text(text));

        // 4. 上下文压缩（超 80% token 限制时）
        let token_estimate = estimate_tokens(&self.messages);
        compact_if_needed(&mut self.messages, token_estimate, 100_000, 10, self.llm.as_ref())?;

        // 5. Agentic loop
        self.agentic_loop(system)?;

        let _ = self.msg_tx.send(AgentMessage::Tokens {
            input: self.input_tokens,
            output: self.output_tokens,
        });
        let _ = self.msg_tx.send(AgentMessage::Done);
        Ok(())
    }

    fn agentic_loop(&mut self, system: crate::agent::prompt::SystemPrompt) -> Result<()> {
        let tools_json = self.tools.to_api_json();

        for _ in 0..self.config.max_iterations {
            // 每次迭代前检查是否被中断
            if let Ok(AgentEvent::Interrupt) = self.event_rx.try_recv() {
                return Ok(());
            }

            // 调用 LLM，流式输出 TextChunk
            let msg_tx = self.msg_tx.clone();
            let stream_result = self.llm.stream_agent(
                &system.blocks,
                &self.messages,
                &tools_json,
                self.config.model_tier.clone(),
                |chunk| {
                    let _ = msg_tx.send(AgentMessage::TextChunk(chunk.to_string()));
                },
            )?;

            self.input_tokens += stream_result.input_tokens;
            self.output_tokens += stream_result.output_tokens;

            // 组装 assistant 消息
            let mut assistant_blocks = stream_result.text_blocks.clone();
            assistant_blocks.extend(stream_result.tool_uses.clone());
            self.messages.push(Message::Assistant(assistant_blocks));

            // 无工具调用 → 本轮结束
            if stream_result.tool_uses.is_empty() || stream_result.stop_reason == "end_turn" {
                break;
            }

            // 执行工具调用
            let tool_uses = stream_result.tool_uses.clone();
            let mut tool_results = Vec::new();

            for tu in &tool_uses {
                if let ContentBlock::ToolUse { id, name, input } = tu {
                    let allowed = self.check_permission(name, input)?;

                    let result = if allowed {
                        match self.tools.get(name) {
                            None => Err(crate::Error::Tool(format!("unknown tool: {}", name))),
                            Some(tool) => {
                                let _ = self.msg_tx.send(AgentMessage::ToolStart {
                                    name: name.clone(),
                                    input: format_tool_input(input),
                                });
                                tool.execute(input)
                            }
                        }
                    } else {
                        Err(crate::Error::Tool("permission denied".into()))
                    };

                    let (content, is_error) = match &result {
                        Ok(out) => {
                            let _ = self.msg_tx.send(AgentMessage::ToolEnd {
                                name: name.clone(),
                                output: out.chars().take(200).collect(),
                            });
                            (out.clone(), false)
                        }
                        Err(e) => {
                            let _ = self.msg_tx.send(AgentMessage::ToolError {
                                name: name.clone(),
                                error: e.to_string(),
                            });
                            (e.to_string(), true)
                        }
                    };

                    tool_results.push(ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content: vec![ContentBlock::Text(content)],
                        is_error,
                    });
                }
            }

            self.messages.push(Message::User(tool_results));
        }

        Ok(())
    }

    /// 检查工具权限。ReadOnly 直接通过；已授权直接通过；否则通过 channel 询问 UI。
    fn check_permission(&mut self, tool_name: &str, input: &JsonValue) -> Result<bool> {
        if self.config.permission_mode == PermissionMode::Bypass {
            return Ok(true);
        }

        // 检查是否 ReadOnly（不持有 borrow 跨越 mut borrow）
        let is_readonly = self.tools.get(tool_name)
            .map(|t| t.permission_level() == PermissionLevel::ReadOnly)
            .unwrap_or(false);

        if is_readonly {
            return Ok(true);
        }

        if self.permissions.is_allowed(tool_name) {
            return Ok(true);
        }

        if self.config.permission_mode == PermissionMode::Auto {
            self.permissions.grant(tool_name);
            return Ok(true);
        }

        // Default 模式：通过 channel 请求 UI 询问用户
        let _ = self.msg_tx.send(AgentMessage::PermissionRequest {
            tool: tool_name.to_string(),
            input: format_tool_input(input),
        });

        loop {
            match self.event_rx.recv() {
                Ok(AgentEvent::PermissionResponse(allowed)) => {
                    if allowed {
                        self.permissions.grant(tool_name);
                    }
                    return Ok(allowed);
                }
                Ok(AgentEvent::Interrupt) => return Ok(false),
                Ok(_) => continue,
                Err(_) => return Ok(false),
            }
        }
    }

    fn evolve(&mut self) -> Result<()> {
        let mut idx = self.index.lock().unwrap();
        evolve_from_session(&self.messages, &self.store, &mut idx, &self.llm)?;
        Ok(())
    }
}

fn format_tool_input(input: &JsonValue) -> String {
    match input {
        JsonValue::Object(pairs) => pairs
            .iter()
            .take(2)
            .map(|(k, v)| {
                let val = match v {
                    JsonValue::Str(s) => {
                        format!("\"{}\"", s.chars().take(40).collect::<String>())
                    }
                    other => format!("{}", other).chars().take(40).collect::<String>(),
                };
                format!("{}={}", k, val)
            })
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}
```

- [ ] **Step 3: 验证编译**

```bash
cd /data/dlab/viv && cargo build 2>&1 | head -40
```
Expected: 可能有 unused import 警告，但无错误（context.rs / run.rs 暂时仍在）

- [ ] **Step 4: Commit**

```bash
cd /data/dlab/viv && git add src/agent/agent.rs src/agent/mod.rs && git commit -m "feat(agent): unified Agent struct with channel-based event loop"
```

---

## Task 4: 创建 `TerminalUI`（`src/bus/terminal.rs`）

**Files:**
- Create: `src/bus/terminal.rs`（从 `src/repl.rs` 迁移重构）

- [ ] **Step 1: 实现 `src/bus/terminal.rs`**

```rust
use std::sync::mpsc::{Receiver, Sender};
use crate::bus::{AgentEvent, AgentMessage};
use crate::core::terminal::backend::{Backend, LinuxBackend};
use crate::core::terminal::buffer::char_width;
use crate::core::terminal::events::{Event, EventLoop};
use crate::core::terminal::input::KeyEvent;
use crate::core::terminal::style::theme;
use crate::tui::block::{Block, BorderSides, BorderStyle};
use crate::tui::header::HeaderWidget;
use crate::tui::input::InputWidget;
use crate::tui::layout::{Constraint, Direction, Layout};
use crate::tui::message_style::{
    format_assistant_message, format_error_message, format_user_message, format_welcome,
};
use crate::tui::permission::{render_permission_pending, render_permission_result};
use crate::tui::paragraph::{Line, Paragraph, Span};
use crate::tui::renderer::Renderer;
use crate::tui::spinner::{random_verb, Spinner};
use crate::tui::status::StatusWidget;
use crate::tui::widget::Widget;

pub struct TerminalUI {
    event_tx: Sender<AgentEvent>,
    msg_rx: Receiver<AgentMessage>,
    backend: LinuxBackend,
    renderer: Renderer,
    editor: LineEditor,
    history_lines: Vec<Line>,
    scroll: u16,
    model_name: String,
    input_tokens: u64,
    output_tokens: u64,
    header: HeaderWidget,
    // 运行时状态
    busy: bool,
    spinner: Spinner,
    spinner_start: Option<std::time::Instant>,
    spinner_verb: String,
    response_line_idx: Option<usize>,
    current_response: String,
    pending_permission: Option<(usize, String, String)>, // (line_idx, tool, input_summary)
}

impl TerminalUI {
    pub fn new(
        event_tx: Sender<AgentEvent>,
        msg_rx: Receiver<AgentMessage>,
    ) -> crate::Result<Self> {
        let mut backend = LinuxBackend::new();
        backend.enter_alt_screen()?;
        backend.enable_raw_mode()?;
        backend.flush()?;

        let size = backend.size()?;
        let renderer = Renderer::new(size);
        let header = HeaderWidget::from_env();
        let spinner_verb = random_verb(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        );

        let mut history_lines: Vec<Line> = Vec::new();
        history_lines.push(format_welcome(&header.cwd, header.branch.as_deref()));
        history_lines.push(Line::raw(""));

        Ok(TerminalUI {
            event_tx,
            msg_rx,
            backend,
            renderer,
            editor: LineEditor::new(),
            history_lines,
            scroll: 0,
            model_name: String::new(), // 等待 Ready 消息
            input_tokens: 0,
            output_tokens: 0,
            header,
            busy: false,
            spinner: Spinner::new(),
            spinner_start: None,
            spinner_verb,
            response_line_idx: None,
            current_response: String::new(),
            pending_permission: None,
        })
    }

    pub fn run(mut self) -> crate::Result<()> {
        let mut event_loop = EventLoop::new()?;
        let mut dirty = true;
        let mut last_cursor: (u16, u16) = (0, 0);

        loop {
            // 消费所有待处理的 Agent 消息
            while let Ok(msg) = self.msg_rx.try_recv() {
                self.handle_agent_message(msg);
                dirty = true;
            }

            if dirty {
                self.render_frame();
                self.renderer.flush(&mut self.backend)?;

                // 定位光标到输入框
                let area = self.renderer.area();
                let input_height = (self.editor.line_count() as u16 + 2).min(8).max(3);
                let chunks = main_layout(input_height).split(area);
                let input_block = Block::new()
                    .border(BorderStyle::Rounded)
                    .borders(BorderSides::HORIZONTAL)
                    .border_fg(theme::DIM);
                let input_inner = input_block.inner(chunks[2]);
                let editor_content = self.editor.content();
                let input_widget =
                    InputWidget::new(&editor_content, self.editor.cursor_offset(), "\u{276F} ")
                        .prompt_fg(theme::CLAUDE);
                let (cx, cy) = input_widget.cursor_position(input_inner);
                if (cx, cy) != last_cursor {
                    self.backend.move_cursor(cy, cx)?;
                    last_cursor = (cx, cy);
                }
                self.backend.show_cursor()?;
                self.backend.flush()?;
                dirty = false;
            }

            // 轮询终端事件（~60fps）
            let events = event_loop.poll(16)?;
            for event in events {
                match event {
                    Event::Key(key) => {
                        dirty = true;
                        if let Some(action) = self.handle_key(key) {
                            match action {
                                UiAction::Quit => {
                                    self.backend.disable_raw_mode()?;
                                    self.backend.leave_alt_screen()?;
                                    self.backend.write(b"Bye!\n")?;
                                    self.backend.flush()?;
                                    let _ = self.event_tx.send(AgentEvent::Quit);
                                    return Ok(());
                                }
                            }
                        }
                    }
                    Event::Resize(new_size) => {
                        self.renderer.resize(new_size);
                        self.scroll = compute_max_scroll(
                            &self.history_lines, &self.renderer, &self.editor,
                        );
                        dirty = true;
                    }
                    Event::Tick => {}
                }
            }
        }
    }

    fn handle_agent_message(&mut self, msg: AgentMessage) {
        match msg {
            AgentMessage::Ready { model } => {
                self.model_name = model;
            }

            AgentMessage::Thinking => {
                self.busy = true;
                self.current_response = String::new();
                self.spinner_start = Some(std::time::Instant::now());
                self.spinner_verb = random_verb(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0),
                );
                let elapsed = 0u64;
                self.history_lines.push(Line::from_spans(vec![
                    Span::styled(
                        format!("{} ", self.spinner.frame_at(elapsed)),
                        theme::CLAUDE,
                        false,
                    ),
                    Span::styled(
                        format!("{}\u{2026}", self.spinner_verb),
                        theme::DIM,
                        false,
                    ),
                ]));
                self.response_line_idx = Some(self.history_lines.len() - 1);
                self.scroll = compute_max_scroll(
                    &self.history_lines, &self.renderer, &self.editor,
                );
            }

            AgentMessage::TextChunk(chunk) => {
                self.current_response.push_str(&chunk);
                if let Some(idx) = self.response_line_idx {
                    // 将 spinner 行替换为实际响应文本
                    let lines = format_assistant_message(&self.current_response);
                    self.history_lines.truncate(idx);
                    self.history_lines.extend(lines);
                    self.scroll = compute_max_scroll(
                        &self.history_lines, &self.renderer, &self.editor,
                    );
                }
            }

            AgentMessage::Status(msg) => {
                // 在 spinner 行下方插入状态提示（不替换响应位置）
                if let Some(idx) = self.response_line_idx {
                    let line = Line::from_spans(vec![
                        Span::styled("  ", theme::DIM, false),
                        Span::styled(msg, theme::DIM, false),
                    ]);
                    if idx + 1 < self.history_lines.len() {
                        self.history_lines[idx + 1] = line;
                    } else {
                        self.history_lines.push(line);
                    }
                }
            }

            AgentMessage::ToolStart { name, input } => {
                let summary = format!("  {} {}", name, input);
                self.history_lines.push(Line::from_spans(vec![
                    Span::styled("\u{25B6} ", theme::CLAUDE, false),
                    Span::styled(summary, theme::DIM, false),
                ]));
                self.scroll = compute_max_scroll(
                    &self.history_lines, &self.renderer, &self.editor,
                );
            }

            AgentMessage::ToolEnd { name: _, output: _ } => {
                // 工具结果已在 ToolStart 行后续响应中体现，此处不额外渲染
            }

            AgentMessage::ToolError { name, error } => {
                let msg = format!("  {} error: {}", name, error);
                self.history_lines.extend(format_error_message(&msg));
                self.scroll = compute_max_scroll(
                    &self.history_lines, &self.renderer, &self.editor,
                );
            }

            AgentMessage::PermissionRequest { tool, input } => {
                self.history_lines.push(render_permission_pending(&tool, &input));
                let idx = self.history_lines.len() - 1;
                self.pending_permission = Some((idx, tool, input));
                self.scroll = compute_max_scroll(
                    &self.history_lines, &self.renderer, &self.editor,
                );
            }

            AgentMessage::Tokens { input, output } => {
                self.input_tokens = input;
                self.output_tokens = output;
            }

            AgentMessage::Done => {
                self.busy = false;
                self.response_line_idx = None;
                self.history_lines.push(Line::raw(""));
                self.scroll = compute_max_scroll(
                    &self.history_lines, &self.renderer, &self.editor,
                );
            }

            AgentMessage::Evolved => {
                // Agent 演化完毕，UI 侧已在 Quit 处理中退出
            }

            AgentMessage::Error(e) => {
                self.history_lines.extend(format_error_message(&format!("error: {}", e)));
                self.busy = false;
                self.response_line_idx = None;
                self.scroll = compute_max_scroll(
                    &self.history_lines, &self.renderer, &self.editor,
                );
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<UiAction> {
        // 权限等待状态：y/n 响应
        if self.pending_permission.is_some() {
            match key {
                KeyEvent::Char('y') | KeyEvent::Char('Y') => {
                    let (idx, tool, summary) = self.pending_permission.take().unwrap();
                    self.history_lines[idx] = render_permission_result(&tool, &summary, true);
                    let _ = self.event_tx.send(AgentEvent::PermissionResponse(true));
                    return None;
                }
                KeyEvent::Char('n') | KeyEvent::Char('N') => {
                    let (idx, tool, summary) = self.pending_permission.take().unwrap();
                    self.history_lines[idx] = render_permission_result(&tool, &summary, false);
                    let _ = self.event_tx.send(AgentEvent::PermissionResponse(false));
                    return None;
                }
                _ => return None,
            }
        }

        // 忙碌状态：只响应 Ctrl+C（中断）
        if self.busy {
            if let KeyEvent::CtrlC = key {
                let _ = self.event_tx.send(AgentEvent::Interrupt);
            }
            return None;
        }

        // 空闲状态：正常编辑
        let action = self.editor.handle_key(key);
        match action {
            EditAction::Submit(line) => {
                if line.trim().is_empty() {
                    return None;
                }
                self.history_lines.push(format_user_message(&line));
                self.scroll = compute_max_scroll(
                    &self.history_lines, &self.renderer, &self.editor,
                );
                let _ = self.event_tx.send(AgentEvent::Input(line));
            }
            EditAction::Exit => {
                return Some(UiAction::Quit);
            }
            EditAction::Interrupt => {
                self.editor.lines = vec![String::new()];
                self.editor.row = 0;
                self.editor.col = 0;
            }
            EditAction::Continue => {}
        }
        None
    }

    fn render_frame(&mut self) {
        let area = self.renderer.area();
        let input_height = (self.editor.line_count() as u16 + 2).min(8).max(3);
        let chunks = main_layout(input_height).split(area);
        let buf = self.renderer.buffer_mut();

        self.header.render(chunks[0], buf);

        // 忙碌时动画 spinner
        if self.busy {
            if let (Some(idx), Some(start)) = (self.response_line_idx, self.spinner_start) {
                let elapsed = start.elapsed().as_millis() as u64;
                if self.current_response.is_empty() && idx < self.history_lines.len() {
                    self.history_lines[idx] = Line::from_spans(vec![
                        Span::styled(
                            format!("{} ", self.spinner.frame_at(elapsed)),
                            theme::CLAUDE,
                            false,
                        ),
                        Span::styled(
                            format!("{}\u{2026}", self.spinner_verb),
                            theme::DIM,
                            false,
                        ),
                    ]);
                }
            }
        }

        let paragraph = Paragraph::new(self.history_lines.clone()).scroll(self.scroll);
        paragraph.render(chunks[1], buf);

        let input_block = Block::new()
            .border(BorderStyle::Rounded)
            .borders(BorderSides::HORIZONTAL)
            .border_fg(theme::DIM);
        let input_inner = input_block.inner(chunks[2]);
        input_block.render(chunks[2], buf);

        let editor_content = self.editor.content();
        let input_widget =
            InputWidget::new(&editor_content, self.editor.cursor_offset(), "\u{276F} ")
                .prompt_fg(theme::CLAUDE);
        input_widget.render(input_inner, buf);

        let status = StatusWidget {
            model: self.model_name.clone(),
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
        };
        status.render(chunks[3], buf);
    }
}

enum UiAction {
    Quit,
}

// ── Layout helpers ────────────────────────────────────────────────────────────

fn main_layout(input_height: u16) -> Layout {
    Layout::new(Direction::Vertical).constraints(vec![
        Constraint::Fixed(1),
        Constraint::Fill,
        Constraint::Fixed(input_height),
        Constraint::Fixed(1),
    ])
}

fn compute_max_scroll(history_lines: &[Line], renderer: &Renderer, editor: &LineEditor) -> u16 {
    let area = renderer.area();
    let input_height = (editor.line_count() as u16 + 2).min(8).max(3);
    let chunks = main_layout(input_height).split(area);
    let conv_height = chunks[1].height as usize;
    let conv_width = chunks[1].width as usize;

    if conv_width == 0 || conv_height == 0 {
        return 0;
    }

    let mut total_rows: usize = 0;
    for line in history_lines {
        total_rows += count_wrapped_rows(line, conv_width);
    }

    if total_rows > conv_height {
        (total_rows - conv_height) as u16
    } else {
        0
    }
}

fn count_wrapped_rows(line: &Line, width: usize) -> usize {
    if width == 0 {
        return 0;
    }
    let total_width: usize = line.spans.iter()
        .flat_map(|s| s.text.chars())
        .map(|c| char_width(c) as usize)
        .sum();
    if total_width == 0 { 1 } else { total_width.div_ceil(width) }
}

// ── LineEditor ────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
pub enum EditAction {
    Continue,
    Submit(String),
    Exit,
    Interrupt,
}

pub struct LineEditor {
    pub lines: Vec<String>,
    pub row: usize,
    pub col: usize,
}

impl LineEditor {
    pub fn new() -> Self {
        LineEditor { lines: vec![String::new()], row: 0, col: 0 }
    }

    pub fn content(&self) -> String { self.lines.join("\n") }
    pub fn cursor_offset(&self) -> usize {
        let prefix: usize = self.lines[..self.row].iter().map(|l| l.len() + 1).sum();
        prefix + self.col
    }
    pub fn line_count(&self) -> usize { self.lines.len() }
    pub fn is_empty(&self) -> bool { self.lines.len() == 1 && self.lines[0].is_empty() }

    pub fn handle_key(&mut self, key: KeyEvent) -> EditAction {
        match key {
            KeyEvent::Char(ch) => {
                self.lines[self.row].insert(self.col, ch);
                self.col += ch.len_utf8();
                EditAction::Continue
            }
            KeyEvent::ShiftEnter => {
                let rest = self.lines[self.row].split_off(self.col);
                self.lines.insert(self.row + 1, rest);
                self.row += 1;
                self.col = 0;
                EditAction::Continue
            }
            KeyEvent::Enter => {
                let content = self.content();
                self.lines = vec![String::new()];
                self.row = 0;
                self.col = 0;
                EditAction::Submit(content)
            }
            KeyEvent::Backspace => {
                if self.col > 0 {
                    let prev = self.prev_char_boundary();
                    self.lines[self.row].drain(prev..self.col);
                    self.col = prev;
                } else if self.row > 0 {
                    let current = self.lines.remove(self.row);
                    self.row -= 1;
                    self.col = self.lines[self.row].len();
                    self.lines[self.row].push_str(&current);
                }
                EditAction::Continue
            }
            KeyEvent::Delete => {
                if self.col < self.lines[self.row].len() {
                    let next = self.next_char_boundary();
                    self.lines[self.row].drain(self.col..next);
                } else if self.row + 1 < self.lines.len() {
                    let next_line = self.lines.remove(self.row + 1);
                    self.lines[self.row].push_str(&next_line);
                }
                EditAction::Continue
            }
            KeyEvent::Left => {
                if self.col > 0 { self.col = self.prev_char_boundary(); }
                else if self.row > 0 { self.row -= 1; self.col = self.lines[self.row].len(); }
                EditAction::Continue
            }
            KeyEvent::Right => {
                if self.col < self.lines[self.row].len() { self.col = self.next_char_boundary(); }
                else if self.row + 1 < self.lines.len() { self.row += 1; self.col = 0; }
                EditAction::Continue
            }
            KeyEvent::Up => {
                if self.row > 0 {
                    self.row -= 1;
                    self.col = self.col.min(self.lines[self.row].len());
                    while self.col > 0 && !self.lines[self.row].is_char_boundary(self.col) {
                        self.col -= 1;
                    }
                }
                EditAction::Continue
            }
            KeyEvent::Down => {
                if self.row + 1 < self.lines.len() {
                    self.row += 1;
                    self.col = self.col.min(self.lines[self.row].len());
                    while self.col > 0 && !self.lines[self.row].is_char_boundary(self.col) {
                        self.col -= 1;
                    }
                }
                EditAction::Continue
            }
            KeyEvent::Home => { self.col = 0; EditAction::Continue }
            KeyEvent::End => { self.col = self.lines[self.row].len(); EditAction::Continue }
            KeyEvent::CtrlC => EditAction::Interrupt,
            KeyEvent::CtrlD => {
                if self.is_empty() { EditAction::Exit } else { EditAction::Continue }
            }
            _ => EditAction::Continue,
        }
    }

    fn prev_char_boundary(&self) -> usize {
        let mut pos = self.col.saturating_sub(1);
        while pos > 0 && !self.lines[self.row].is_char_boundary(pos) { pos -= 1; }
        pos
    }
    fn next_char_boundary(&self) -> usize {
        let line = &self.lines[self.row];
        let mut pos = self.col + 1;
        while pos < line.len() && !line.is_char_boundary(pos) { pos += 1; }
        pos
    }
}

impl Default for LineEditor {
    fn default() -> Self { Self::new() }
}
```

- [ ] **Step 2: 验证编译**

```bash
cd /data/dlab/viv && cargo build 2>&1 | grep "^error" | head -20
```
Expected: 无 error（可能有 unused 警告）

- [ ] **Step 3: Commit**

```bash
cd /data/dlab/viv && git add src/bus/terminal.rs && git commit -m "feat(bus): implement TerminalUI with channel-based event handling"
```

---

## Task 5: 更新模块接线（`main.rs`、`lib.rs`、`agent/mod.rs`）

**Files:**
- Modify: `src/main.rs`
- Modify: `src/lib.rs`
- Modify: `src/agent/mod.rs`

- [ ] **Step 1: 更新 `src/main.rs`**

```rust
use std::sync::mpsc::channel;
use std::thread;
use viv::agent::agent::{Agent, AgentConfig};
use viv::bus::{AgentEvent, AgentMessage};
use viv::bus::terminal::TerminalUI;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> viv::Result<()> {
    let (event_tx, event_rx) = channel::<AgentEvent>();
    let (msg_tx, msg_rx) = channel::<AgentMessage>();

    let config = AgentConfig::default();
    let agent = Agent::new(config, event_rx, msg_tx)?;

    // Agent 在独立线程无限循环
    let handle = thread::spawn(move || agent.run());

    // TerminalUI 在主线程运行
    TerminalUI::new(event_tx, msg_rx)?.run()?;

    handle.join().unwrap_or(Ok(()))
}
```

- [ ] **Step 2: 更新 `src/lib.rs`**

```rust
pub mod agent;
pub mod bus;
pub mod core;
pub mod error;
pub mod llm;
pub mod memory;
pub mod permissions;
pub mod tools;
pub mod tui;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;
```

（移除 `pub mod repl;`）

- [ ] **Step 3: 验证编译**

```bash
cd /data/dlab/viv && cargo build 2>&1 | grep "^error" | head -20
```
Expected: 无 error

- [ ] **Step 4: Commit**

```bash
cd /data/dlab/viv && git add src/main.rs src/lib.rs && git commit -m "feat(main): wire Agent + TerminalUI via channels, spawn Agent thread"
```

---

## Task 6: 删除旧文件，清理模块声明

**Files:**
- Delete: `src/agent/run.rs`
- Delete: `src/agent/context.rs`
- Delete: `src/repl.rs`
- Delete: `tests/agent/run_test.rs`
- Delete: `tests/repl_test.rs`
- Modify: `src/agent/mod.rs`

- [ ] **Step 1: 更新 `src/agent/mod.rs`（移除旧模块）**

```rust
pub mod agent;
pub mod evolution;
pub mod message;
pub mod prompt;
```

（移除 `pub mod context;` 和 `pub mod run;`）

- [ ] **Step 2: 删除旧源文件**

```bash
rm /data/dlab/viv/src/agent/run.rs
rm /data/dlab/viv/src/agent/context.rs
rm /data/dlab/viv/src/repl.rs
```

- [ ] **Step 3: 删除旧测试文件**

```bash
rm /data/dlab/viv/tests/agent/run_test.rs
rm /data/dlab/viv/tests/repl_test.rs
```

- [ ] **Step 4: 更新 `tests/agent/mod.rs`（移除 run_test 引用）**

打开 `tests/agent/mod.rs`，移除 `mod run_test;` 一行。同时移除顶层 `tests/agent_tests.rs` 或 `tests/core_tests.rs` 中的 `mod repl;` 引用（如存在）。

- [ ] **Step 5: 完整编译验证**

```bash
cd /data/dlab/viv && cargo build 2>&1
```
Expected: 无 error，有少量 unused 警告可忽略

- [ ] **Step 6: 运行所有测试**

```bash
cd /data/dlab/viv && cargo test 2>&1
```
Expected: 原有测试通过，新增 bus + permissions 测试通过

- [ ] **Step 7: Commit**

```bash
cd /data/dlab/viv && git add -A && git commit -m "refactor(agent): remove context.rs/run.rs/repl.rs, wire unified Agent"
```

---

## Task 7: 构建验证与 Clippy

- [ ] **Step 1: Release 构建**

```bash
cd /data/dlab/viv && cargo build --release 2>&1
```
Expected: 编译成功

- [ ] **Step 2: Clippy 检查**

```bash
cd /data/dlab/viv && cargo clippy 2>&1 | grep "^error" | head -20
```
Expected: 无 error

- [ ] **Step 3: 完整测试套件**

```bash
cd /data/dlab/viv && cargo test 2>&1
```
Expected: 所有测试通过

- [ ] **Step 4: 最终 Commit**

```bash
cd /data/dlab/viv && git add -A && git commit -m "chore: verify Agent refactor builds and all tests pass"
```
