# Tool System Overhaul Design

完善 viv 的 tool 支持，对齐 Claude Code 实现。

## 目标

| 维度 | 内容 |
|------|------|
| A 名称对齐 | `FileRead→Read`, `FileWrite→Write`, `FileEdit→Edit` |
| B Description | 所有 tool 对齐 Claude Code 级别的详细描述 |
| C 参数补全 | 修复 Glob 排序、实现 Read pages、Grep 可移植性、TodoWrite 格式对齐等 |
| D 新增 tool | NotebookEdit、Agent（SubAgent 并发）、WebSearch（Tavily） |
| E 通信抽象 | Agent 间双向通信的 channel 抽象 |

## 实现策略

Top-Down：先改框架（通信抽象 → Tool trait / ToolRegistry → Agent loop），再逐个填充 tool 细节。

## 通信架构总览

三条规则：

1. **主 Agent ↔ UI**：通过现有的 `AgentEvent` / `AgentMessage` + `mpsc` 通信，**不改动**
2. **主 Agent ↔ 子 Agent**：通过新的 `agent_channel()` 双向通信
3. **子 Agent ↔ 子 Agent**：不允许（子 Agent 只和创建它的父 Agent 通信）

```
┌──────────┐  AgentEvent/AgentMessage  ┌────────────┐  agent_channel  ┌─────────────┐
│    UI    │◄─────────────────────────►│  主 Agent  │◄──────────────►│  子 Agent A │
│(Terminal)│   (现有 mpsc, 不改动)      │            │                └─────────────┘
└──────────┘                           │            │  agent_channel  ┌─────────────┐
                                       │            │◄──────────────►│  子 Agent B │
                                       └────────────┘                └─────────────┘
                                                                      (A 和 B 之间
                                                                       不能通信)
```

---

## Part 1: Agent 间通信抽象

### 1.1 agent_channel — 仅用于 Agent 间通信

```rust
// src/bus/channel.rs (新文件)

use crate::bus::{AgentEvent, AgentMessage};
use crate::core::runtime::channel::{async_channel, NotifySender, AsyncReceiver};

/// 父 Agent 侧 — 向子 Agent 发指令，收子 Agent 的消息
pub struct AgentHandle {
    pub tx: NotifySender<AgentEvent>,
    pub rx: std::sync::mpsc::Receiver<AgentMessage>,
}

/// 子 Agent 侧 — 收父 Agent 的指令，发消息给父 Agent
pub struct AgentEndpoint {
    pub rx: AsyncReceiver<AgentEvent>,
    pub tx: std::sync::mpsc::Sender<AgentMessage>,
}

/// 创建 Agent 间通信的一对端点
pub fn agent_channel() -> (AgentHandle, AgentEndpoint) {
    let (event_tx, event_rx) = async_channel();
    let (msg_tx, msg_rx) = std::sync::mpsc::channel();
    (
        AgentHandle { tx: event_tx, rx: msg_rx },
        AgentEndpoint { rx: event_rx, tx: msg_tx },
    )
}
```

### 1.2 主 Agent 不变

主 Agent 保持现有结构，UI 通信路径完全不动：

```rust
pub struct Agent {
    event_rx: AsyncReceiver<AgentEvent>,   // 收来自 UI 的指令（不变）
    msg_tx: Sender<AgentMessage>,          // 发给 UI 的消息（不变）
    // ...
}
```

`main.rs`、`TerminalUI` — **零改动**。

### 1.3 子 Agent 使用 AgentEndpoint

子 Agent 通过 `AgentEndpoint` 与父 Agent 通信：

```rust
// Agent::new_sub() 接收 AgentEndpoint
pub async fn new_sub(
    config: AgentConfig,
    endpoint: AgentEndpoint,
    llm: Arc<LLMClient>,
) -> Result<Self>
```

子 Agent 内部用 `endpoint.rx` 收指令（如权限响应、中断），用 `endpoint.tx` 发消息（如文本输出、权限请求、完成通知）。

### 1.4 双向通信场景

| 场景 | 子 Agent 发 | 父 Agent 收到后 |
|------|------------|----------------|
| 请求权限 | `AgentMessage::PermissionRequest { tool, input }` | 决策后回 `AgentEvent::PermissionResponse(bool)` |
| 流式输出 | `AgentMessage::TextChunk(text)` | 收集文本 |
| 状态汇报 | `AgentMessage::Status(msg)` | 记录或忽略 |
| 被中断 | — | 父 Agent 发 `AgentEvent::Interrupt` |
| 完成 | `AgentMessage::Done` | 结束监听 |

### 1.5 隔离性保证

子 Agent 之间不能通信 — 每个子 Agent 只持有一个 `AgentEndpoint`，只能和创建它的父 Agent 交互。父 Agent 持有每个子 Agent 的 `AgentHandle`，是唯一的协调者。

未来扩展（如上下文请求）只需给 `AgentEvent` / `AgentMessage` 枚举加 variant。

---

## Part 2: Tool trait 与 ToolRegistry

### 2.1 Tool trait

保持不变：

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> JsonValue;
    fn execute(&self, input: &JsonValue)
        -> Pin<Box<dyn Future<Output = Result<String>> + Send + '_>>;
    fn permission_level(&self) -> PermissionLevel;
}
```

### 2.2 ToolRegistry 改进

Agent 中 `tools: ToolRegistry` 保持不变（不需要 Arc）。

**to_api_json() 改进**：内部用 `JsonValue` 构建代替字符串拼接（避免转义问题），仍返回 `String`。调用方（`llm.rs`）无需变更。

**新增 `default_tools_without()`**：供子 Agent 创建不含指定 tool 的 registry：

```rust
impl ToolRegistry {
    pub fn default_tools_without(exclude: &str, llm: Arc<LLMClient>) -> Self {
        let mut reg = Self::default_tools(llm);
        reg.tools.retain(|t| t.name() != exclude);
        reg
    }
}
```

### 2.3 Tool 名称重映射

| 之前 | 之后 |
|------|------|
| `FileRead` | `Read` |
| `FileWrite` | `Write` |
| `FileEdit` | `Edit` |

`agent.rs` 中 LSP 通知判断同步更新：

```rust
// 之前
if matches!(name.as_str(), "FileEdit" | "FileWrite" | "MultiEdit")
// 之后
if matches!(name.as_str(), "Edit" | "Write" | "MultiEdit")
```

---

## Part 3: Agent Loop 并发与 SubAgent

### 3.1 Agent Loop 并发改造

当 LLM 返回多个 tool_use 时，Agent tool 并发执行，普通 tool 串行执行：

```rust
let (agent_tasks, normal_tasks): (Vec<_>, Vec<_>) =
    tool_uses.iter().partition(|tu| tu.name == "Agent");

// 1. 串行执行普通 tool
for tu in &normal_tasks {
    let result = tool.execute(input).await;
    tool_results.push(result);
}

// 2. 并发执行 Agent tool
let agent_futures: Vec<_> = agent_tasks.iter()
    .map(|tu| tool.execute(input))
    .collect();
let agent_results = join_all(agent_futures).await;
tool_results.extend(agent_results);
```

### 3.2 join_all 实现

在 `core::runtime` 中添加零依赖的 `join_all` 和 `join`：

```rust
pub async fn join_all<F, T>(futures: Vec<F>) -> Vec<T>
where F: Future<Output = T> + Send

pub async fn join<A, B, RA, RB>(a: A, b: B) -> (RA, RB)
where A: Future<Output = RA> + Send, B: Future<Output = RB> + Send
```

基于 viv 已有的单线程 async runtime，轮询所有 future 直到全部完成。

### 3.3 SubAgentTool 结构

```rust
pub struct SubAgentTool {
    llm: Arc<LLMClient>,
}
```

文件位置：`src/tools/agent.rs`

- **名称**: `"Agent"`
- **权限**: `ReadOnly`

### 3.4 参数

```json
{
  "prompt": "string, required — 子 Agent 执行的任务描述",
  "model": "string, optional — fast|medium|slow, 默认 fast",
  "max_iterations": "number, optional — 最大迭代次数, 默认 20"
}
```

### 3.5 执行流程

SubAgent 是临时的、轻量的 — 通过 `agent_channel()` 创建通信端点，用完即毁：

```rust
async fn execute(&self, input: &JsonValue) -> Result<String> {
    let prompt = /* 从 input 解析 */;
    let tier = /* 从 input 解析，默认 Fast */;
    let max_iter = /* 从 input 解析，默认 20 */;

    // 创建 agent 间通信 channel
    let (handle, endpoint) = agent_channel();

    // 创建子 Agent（轻量版 — 无 MCP/LSP/Memory）
    let config = AgentConfig {
        model_tier: tier,
        max_iterations: max_iter,
        permission_mode: PermissionMode::Auto,
        ..Default::default()
    };
    let child = Agent::new_sub(config, endpoint, Arc::clone(&self.llm)).await?;

    // 并发：子 Agent 运行 + 父侧监听消息
    let child_future = child.run();

    let monitor_future = async {
        let mut collected_text = String::new();
        loop {
            match handle.rx.try_recv() {
                Ok(AgentMessage::TextChunk(t)) => collected_text.push_str(&t),
                Ok(AgentMessage::Done) => break,
                Ok(AgentMessage::PermissionRequest { .. }) => {
                    // 双向：子 Agent 请求权限，父 Agent 自动批准
                    let _ = handle.tx.send(AgentEvent::PermissionResponse(true));
                }
                Ok(_) => {}
                Err(TryRecvError::Empty) => yield_now().await,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        Ok(collected_text)
    };

    let (_, text) = join(child_future, monitor_future).await;
    text
}
```

### 3.6 Agent::new_sub() — 轻量子 Agent 构造器

```rust
impl Agent {
    /// 创建子 Agent — 无 MCP/LSP/Memory，纯 tool 执行
    pub async fn new_sub(
        config: AgentConfig,
        endpoint: AgentEndpoint,
        llm: Arc<LLMClient>,
    ) -> Result<Self> {
        let tools = ToolRegistry::default_tools_without("Agent", Arc::clone(&llm));
        Ok(Agent {
            event_rx: endpoint.rx,
            msg_tx: endpoint.tx,
            tools,
            llm,
            config,
            messages: vec![],
            prompt_cache: PromptCache::default(),
            // store, index, mcp, lsp, permissions — 空/默认值
            ..
        })
    }
}
```

子 Agent 复用完整 `Agent` 结构体和 `Agent::run()` 逻辑。`event_rx` / `msg_tx` 指向的是 `agent_channel` 的端点（父 Agent），而非 UI。对 Agent 内部逻辑完全透明。

### 3.7 递归防护

`default_tools_without("Agent")` 排除了 Agent tool，防止无限递归。

---

## Part 4: 现有 Tool 改造

### 4.1 Read（原 FileRead）

**名称变更**: `FileRead` → `Read`

**Description**: 对齐 Claude Code — 强调绝对路径、cat -n 格式、默认 2000 行、offset/limit 使用指导。

**参数补全**:
- `pages`: 检测 `.pdf` 后缀，调用 `pdftotext -f <start> -l <end> <file> -` 命令。`pdftotext` 不存在时返回友好错误
- 二进制文件检测：读前 512 字节检查 NUL 字节，是二进制则返回提示信息而非崩溃

### 4.2 Write（原 FileWrite）

**名称变更**: `FileWrite` → `Write`

**Description**: 对齐 Claude Code — 强调先 Read 再 Write、偏好 Edit、不创建 md。

**实现改进**: 返回信息包含行数。

### 4.3 Edit（原 FileEdit）

**名称变更**: `FileEdit` → `Edit`

**Description**: 对齐 Claude Code — 强调先 Read、保留缩进、old_string 唯一性、replace_all 场景。

无参数变更。

### 4.4 MultiEdit

保留，description 补充原子性说明。无参数变更。

### 4.5 Bash

**Description**: 对齐 Claude Code 详细使用指南 — 不用 bash 做 grep/cat、引号处理、git 注意事项、timeout 说明。

**实现修复**: 从 schema 中移除 `dangerouslyDisableSandbox`（viv 无沙箱机制）。

### 4.6 Glob

**Description 修正**: 移除"sorted by modification time"的错误描述，改为真正按修改时间排序。

**实现修复**:
- `matches.sort()` 改为按 `metadata().modified()` 排序（最近修改的排前面）
- 添加默认忽略：`.git/`, `node_modules/`, `target/`

### 4.7 Grep

**Description**: 对齐 Claude Code — regex 语法说明、output_mode 用途、head_limit 说明。

**参数补全**:
- `context` 字段：添加为 `-C` 的别名（Claude Code 两个都有）
- `multiline` 可移植性：`-z` 是 GNU 扩展，在 macOS 上不可用。改为检测平台：Linux 用 `grep -Pz`，其他平台回退到 `grep -E`（multiline 降级为警告）
- `type` 扩展：添加常用语言的 type→glob 映射表：
  - `js` → `*.{js,jsx,mjs,cjs}`
  - `ts` → `*.{ts,tsx,mts,cts}`
  - `py` → `*.{py,pyi}`
  - `rs` → `*.rs`
  - `go` → `*.go`
  - 等

### 4.8 LS

保留，description 丰富。无实现变更。

### 4.9 TodoWrite

**格式对齐 Claude Code**:
- 移除 `id` 字段
- 移除 `priority` 字段
- 添加 `activeForm` 字段（进行时描述，如 "Running tests"）
- 保持 `content` + `status`（pending | in_progress | completed）

### 4.10 TodoRead

保留（viv 独有），无变更。

### 4.11 WebFetch

**Description**: 对齐 Claude Code — 不支持认证 URL、HTTP→HTTPS 升级。

**实现改进**:
- HTML → Markdown 转换器替代 `strip_html()`：处理 `<h1-6>` → `#`、`<a>` → `[text](url)`、`<code>/<pre>` → `` ` ``/` ``` `、`<ul>/<li>` → `- `、`<strong>` → `**`、`<em>` → `*`、`<p>` → 换行
- 截断阈值 8000 → 16000 字符

---

## Part 5: 新增 Tool — NotebookEdit

### 5.1 结构

文件位置：`src/tools/notebook.rs`

### 5.2 Tool 接口

- **名称**: `"NotebookEdit"`
- **权限**: `Write`

### 5.3 参数

```json
{
  "notebook_path": "string, required — ipynb 文件的绝对路径",
  "cell_id": "string, optional — cell 的 id 字段，用于定位",
  "cell_type": "string, optional — code|markdown, insert 时 required",
  "edit_mode": "string, optional — replace|insert|delete, 默认 replace",
  "new_source": "string, required — cell 的新内容（delete 时忽略）"
}
```

### 5.4 实现要点

- ipynb 是 JSON：`{ cells: [{ cell_type, source, id?, metadata, outputs }] }`
- 用 `JsonValue` 解析
- `cell_id` 匹配 cell 的 `id` 字段（nbformat 4.5+）；如无 id 字段，按数组索引回退
- replace: 替换 `source` 字段（转为 `["line1\n", "line2\n"]` 格式）
- insert: 在目标 cell 之后插入新 cell，生成 minimal metadata
- delete: 移除目标 cell
- 写回时保持原 JSON 结构（metadata, outputs, nbformat 等不动）

---

## Part 6: 新增 Tool — WebSearch（Tavily）

### 6.1 结构

文件位置：`src/tools/search.rs`

### 6.2 环境变量

`VIV_TAVILY_API_KEY` — required。未设置时 tool 仍注册，execute 时返回友好错误。

### 6.3 Tool 接口

- **名称**: `"WebSearch"`
- **权限**: `ReadOnly`

### 6.4 参数

```json
{
  "query": "string, required — 搜索关键词",
  "max_results": "number, optional — 默认 10, 最大 20",
  "search_depth": "string, optional — basic|advanced, 默认 basic",
  "topic": "string, optional — general|news, 默认 general",
  "include_domains": "array of string, optional",
  "exclude_domains": "array of string, optional"
}
```

### 6.5 实现要点

- POST `https://api.tavily.com/search`
- 复用 `AsyncTlsStream` + `HttpRequest`
- Request body: `{ api_key, query, max_results, search_depth, topic, include_domains, exclude_domains }`
- Response 解析：提取 `results[].{title, url, content}`
- 格式化输出：编号列表，每项 title + url + content snippet

---

## Part 7: default_tools 注册顺序

```rust
pub fn default_tools(llm: Arc<LLMClient>) -> Self {
    let mut reg = ToolRegistry::new();
    // 核心文件操作
    reg.register(Box::new(ReadTool));
    reg.register(Box::new(WriteTool));
    reg.register(Box::new(EditTool));
    reg.register(Box::new(MultiEditTool));
    reg.register(Box::new(NotebookEditTool));
    // 搜索
    reg.register(Box::new(GlobTool));
    reg.register(Box::new(GrepTool));
    reg.register(Box::new(LsTool));
    // 执行
    reg.register(Box::new(BashTool));
    // 任务管理
    reg.register(Box::new(TodoWriteTool::new(todo_path.clone())));
    reg.register(Box::new(TodoReadTool::new(todo_path)));
    // 网络
    reg.register(Box::new(WebFetchTool::new(Arc::clone(&llm))));
    reg.register(Box::new(WebSearchTool));
    // SubAgent — 用时通过 agent_channel() 创建通信端点，跑完即毁
    reg.register(Box::new(SubAgentTool::new(Arc::clone(&llm))));
    reg
}
```

---

## 文件变更清单

| 文件 | 变更类型 |
|------|---------|
| `src/bus/mod.rs` | 修改 — 新增 `pub mod channel` |
| `src/bus/channel.rs` | 新增 — AgentHandle, AgentEndpoint, agent_channel() |
| `src/agent/agent.rs` | 修改 — 新增 new_sub(), LSP 名称更新, Agent tool 并发执行 |
| `src/tools/mod.rs` | 修改 — to_api_json 用 JsonValue 构建, 新增 default_tools_without |
| `src/tools/bash.rs` | 修改 — description 丰富, 移除 dangerouslyDisableSandbox |
| `src/tools/file/read.rs` | 修改 — 改名 Read, description, PDF pages 实现, 二进制检测 |
| `src/tools/file/write.rs` | 修改 — 改名 Write, description, 返回行数 |
| `src/tools/file/edit.rs` | 修改 — 改名 Edit, description (EditTool + MultiEditTool) |
| `src/tools/file/glob.rs` | 修改 — description 修正, 按修改时间排序, 默认忽略目录 |
| `src/tools/file/grep.rs` | 修改 — description, context 别名, type 映射表, multiline 可移植性 |
| `src/tools/file/ls.rs` | 修改 — description 丰富 |
| `src/tools/todo.rs` | 修改 — TodoWrite 格式对齐 (移除 id/priority, 加 activeForm) |
| `src/tools/web.rs` | 修改 — description, HTML→Markdown 转换器, 截断 16000 |
| `src/tools/notebook.rs` | 新增 — NotebookEditTool |
| `src/tools/search.rs` | 新增 — WebSearchTool (Tavily) |
| `src/tools/agent.rs` | 新增 — SubAgentTool, 使用 agent_channel() 双向通信 |
| `src/core/runtime/mod.rs` | 修改 — 新增 join_all, join |
| `tests/bus/channel_test.rs` | 新增 — agent_channel 通信测试 |
| `tests/tools/` | 修改 — 所有引用旧名称的测试更新 |
| `tests/tools/notebook_test.rs` | 新增 |
| `tests/tools/search_test.rs` | 新增 |
| `tests/tools/agent_test.rs` | 新增 |
