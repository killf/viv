# Agent 重构设计文档

**日期：** 2026-04-17  
**状态：** 已批准

## 目标

将 Agent 相关逻辑从多个分散的结构（`AgentContext`、`run_agent()`、`repl.rs`）整合为单一统一的 `Agent` 入口，同时通过双线程 + channel 实现 UI 层与 Agent 层的彻底分离。

---

## 架构概览

```
main.rs
├── channel<AgentEvent>   (UI → Agent)
├── channel<AgentMessage> (Agent → UI)
├── thread::spawn → Agent::run()     // Agent 线程
└── TerminalUI::run()                // UI 主线程（渲染 + 输入）
```

UI 层负责所有渲染任务，Agent 层负责所有业务逻辑。两者仅通过 channel 通信，互不依赖。

---

## 通信协议

### `AgentEvent`（UI → Agent）

```rust
pub enum AgentEvent {
    Input(String),             // 用户提交消息
    PermissionResponse(bool),  // 权限询问的回答
    Interrupt,                 // Ctrl+C，中断当前操作
    Quit,                      // Ctrl+D / /exit
}
```

### `AgentMessage`（Agent → UI）

```rust
pub enum AgentMessage {
    // LLM 流程
    Thinking,                  // 开始处理，UI 显示 spinner
    TextChunk(String),         // LLM 流式输出片段

    // 工具执行
    ToolStart { name: String, input: String },
    ToolEnd   { name: String, output: String },
    ToolError { name: String, error: String },

    // 权限
    PermissionRequest { tool: String, input: String },

    // 状态
    Status(String),            // 进度提示，如"检索记忆(2条)…"

    // 生命周期
    Done,                      // 本轮结束，等待下一条输入
    Evolved,                   // 退出时记忆演化完成
    Error(String),             // 不可恢复错误
}
```

### 完整生命周期示意

```
用户输入 → Input("xxx")
           Thinking
           Status("检索记忆(2条)…")
           TextChunk("...") × N
           ToolStart { name: "bash", input: "ls" }
           PermissionRequest { tool: "bash", input: "ls" }
用户授权 → PermissionResponse(true)
           ToolEnd { name: "bash", output: "..." }
           TextChunk("...") × N
           Done
用户退出 → Quit
           Evolved
```

---

## Agent 结构

```rust
// src/agent/agent.rs
pub struct Agent {
    // 对话状态
    messages: Vec<Message>,
    prompt_cache: PromptCache,

    // 资源层
    llm: Arc<LlmClient>,
    store: Arc<MemoryStore>,
    index: Arc<Mutex<MemoryIndex>>,

    // 工具与权限
    tools: ToolRegistry,
    permissions: PermissionManager,

    // 配置
    config: AgentConfig,

    // 统计
    input_tokens: u64,
    output_tokens: u64,

    // 通信
    event_rx: Receiver<AgentEvent>,
    msg_tx: Sender<AgentMessage>,
}
```

### 主要方法

```rust
impl Agent {
    pub fn new(
        config: AgentConfig,
        event_rx: Receiver<AgentEvent>,
        msg_tx: Sender<AgentMessage>,
    ) -> Result<Self>;

    pub fn run(mut self) -> Result<()>;          // 无限 loop，阻塞

    fn handle_input(&mut self, text: String) -> Result<()>;
    fn agentic_loop(&mut self) -> Result<()>;    // LLM + 工具循环
    fn wait_permission(&mut self) -> bool;       // 阻塞等待 PermissionResponse
    fn evolve(&mut self) -> Result<()>;          // 退出时保存记忆
}
```

### Agent Loop 逻辑

```
Agent::run()
└── loop
    ├── event_rx.recv() 阻塞等待
    ├── Input(text) → handle_input(text)
    │   ├── send(Thinking)
    │   ├── send(Status("检索记忆…"))
    │   ├── retrieve_memories(text)
    │   ├── build_system_prompt()
    │   ├── compact_if_needed()
    │   └── agentic_loop()
    │       ├── llm.stream_agent() → send(TextChunk) × N
    │       ├── for each tool_call:
    │       │   ├── permission check → send(PermissionRequest)
    │       │   │   └── wait_permission() → event_rx.recv() → PermissionResponse
    │       │   ├── tool.execute()
    │       │   │   ├── send(ToolStart)
    │       │   │   └── send(ToolEnd) 或 send(ToolError)
    │       │   └── append tool result to messages
    │       └── stop_reason == end_turn → break
    │       send(Done)
    ├── Interrupt → 中断 agentic_loop，send(Done)
    └── Quit → evolve() → send(Evolved) → break
```

---

## 文件结构变化

### 新增

```
src/
├── bus/
│   ├── mod.rs          # AgentEvent + AgentMessage 定义
│   └── terminal.rs     # TerminalUI 实现（从 repl.rs 迁移）
└── agent/
    └── agent.rs        # Agent struct + 所有方法
```

### 修改

```
src/
├── main.rs             # 建 channel → Agent::new() + TerminalUI::new() → 各跑各线程
└── agent/
    └── evolution.rs    # evolve_from_session() → Agent::evolve() 方法
```

### 删除

```
src/agent/run.rs        # 逻辑合并进 agent.rs
src/agent/context.rs    # 字段直接放进 Agent struct
src/repl.rs             # 拆成 bus/terminal.rs
```

---

## 不变的部分

- `src/agent/message.rs` — Message、ContentBlock 类型
- `src/agent/prompt.rs` — 系统 prompt 构建
- `src/memory/` — 记忆系统
- `src/tools/` — 工具实现
- `src/permissions/` — 权限管理
- `src/llm.rs` — LLM 客户端
- `src/core/` — 底层 IO、网络、终端
