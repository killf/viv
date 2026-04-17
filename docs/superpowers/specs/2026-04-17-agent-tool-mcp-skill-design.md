# viv Agent Loop / Tool / MCP / Skill 设计文档

**日期：** 2026-04-17  
**状态：** 已批准，待实现  
**参考：** /data/github/claude-code-restored

---

## 一、目标

在 viv 现有 REPL + 流式 LLM 客户端基础上，完整实现：

1. **Agent 循环**：tool_use → tool_result → 再次请求，直到 end_turn
2. **Tool 系统**：60+ 内置工具，与 Claude Code 参数/说明严格一致
3. **MCP 协议栈**：stdio / SSE / HTTP / WebSocket 四种传输
4. **Skill 系统**：本地 + 插件 + 自动发现 + Hook 触发，完整复刻 Claude Code
5. **权限模型**：default/auto/bypass 三模式 + 规则匹配 + AI 动态分类器

**核心约束：** 零外部依赖（edition 2024 单 crate），为裸机 AgentOS 部署准备。

---

## 二、整体架构

### 双线程模型

```
┌─────────────────────────────────────────────────────┐
│                   viv Runtime                        │
│                                                      │
│  ┌──────────────┐    channel    ┌─────────────────┐  │
│  │  UI Thread   │◄─────────────►│  Runtime Thread │  │
│  │              │               │                 │  │
│  │  epoll       │  UiEvent      │  Task Executor  │  │
│  │  + render    │  AgentEvent   │  + I/O Reactor  │  │
│  └──────────────┘               │                 │  │
│                                 │  ┌─────────────┐│  │
│                                 │  │ Agent Loop  ││  │
│                                 │  │  (Future)   ││  │
│                                 │  └──────┬──────┘│  │
│                                 │         │        │  │
│                                 │  ┌──────▼──────┐│  │
│                                 │  │ Tool Tasks  ││  │
│                                 │  │ (并发 Future)││  │
│                                 │  └─────────────┘│  │
│                                 └─────────────────┘  │
└─────────────────────────────────────────────────────┘
```

| 线程 | 职责 | 技术 |
|------|------|------|
| UI Thread | 终端输入/渲染，60fps | 现有 epoll loop |
| Runtime Thread | Future executor + I/O reactor | 自制 executor + epoll |

### 新增模块

```
src/
├── runtime/          # 自制 async executor + I/O reactor
├── agent/            # Agent 循环、消息类型、上下文
├── tools/            # Tool trait + 60+ 内置工具
├── mcp/              # MCP 协议栈（4 种传输）
├── skills/           # Skill 加载、发现、Hook
└── permissions/      # 权限规则 + AI 分类器
```

现有 `net/`、`terminal/`、`llm.rs`、`repl.rs` 保持不动，作为底层依赖。

---

## 三、自制 Async Runtime

参考 tokio 核心三组件：Reactor + Executor + Waker。

### Reactor（I/O 事件通知）

复用 `event.rs` 的 epoll，扩展为统一 I/O reactor：
- fd 就绪时唤醒对应 `Waker`
- 支持 readable / writable / timer 三类事件

### Executor（任务调度）

```rust
struct Executor {
    ready_queue: VecDeque<Arc<Task>>,
    tasks: HashMap<TaskId, Arc<Task>>,
}

struct Task {
    id: TaskId,
    future: Mutex<Pin<Box<dyn Future<Output = ()> + Send>>>,
    sender: Sender<TaskId>,
}
```

**运行循环：**

```
loop {
    // 1. 排干就绪队列
    while let Some(task) = ready_queue.pop() {
        task.poll(waker);
    }
    // 2. epoll_wait 阻塞直到 fd/timer 就绪
    reactor.wait() → 唤醒 Waker → task 入队
}
```

### Waker

用标准库 `std::task::{RawWaker, Waker}` 手动实现——唤醒时往 channel 发送 `TaskId`，executor 收到后重新入队。

### 公开 API

```rust
runtime::spawn(future)           // 提交新任务
runtime::block_on(future)        // 阻塞等待
AsyncTcpStream::connect(addr)    // 封装 net/tcp.rs
AsyncTcpStream::read/write       // 注册到 reactor，返回 Future
sleep(Duration)                  // timer future
```

**模型：** 单线程 executor（M:1），所有 Future 在 Runtime Thread 上轮询，工具并发靠 `spawn`。

---

## 四、Agent 循环 & Tool 系统

### Agent 循环

```rust
async fn agent_loop(ctx: &mut AgentContext) {
    loop {
        let response = llm_client.stream(ctx.messages()).await;
        let (text, tool_uses) = collect_response(response).await;

        if tool_uses.is_empty() { break; }

        let results = run_tools(tool_uses, ctx).await;
        ctx.append_tool_results(results);
    }
}

async fn run_tools(tool_uses: Vec<ToolUse>, ctx: &AgentContext) -> Vec<ToolResult> {
    let mut handles = vec![];
    for tu in tool_uses {
        if tu.tool.is_concurrency_safe() {
            handles.push(runtime::spawn(tu.tool.call(tu.input, ctx)));
        } else {
            // 等待前序 spawn 完成后顺序执行
        }
    }
    join_all(handles).await
}
```

### Tool Trait

```rust
trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn aliases(&self) -> &[&str] { &[] }
    fn description(&self) -> String;
    fn input_schema(&self) -> JsonSchema;
    fn is_concurrency_safe(&self) -> bool { true }

    async fn call(&self, input: JsonValue, ctx: &ToolContext) -> ToolResult;
    async fn check_permissions(&self, input: &JsonValue, ctx: &ToolContext) -> Permission;
}
```

### 内置工具分组

工具参数和说明**严格对齐** claude-code-restored 定义，一字不差。

| 分组 | 工具 |
|------|------|
| **Shell** | Bash |
| **文件** | Read, Write, Edit, MultiEdit, NotebookEdit, NotebookRead |
| **搜索** | Glob, Grep, LS |
| **网络** | WebFetch, WebSearch |
| **Agent** | Agent（递归）, Task 系列, Memory |
| **UI** | AskUser, Skill, ExitPlanMode, EnterPlanMode |
| **Git/IDE** | 若干辅助工具 |
| **MCP** | MCP 服务工具代理（动态注册） |

### 消息类型

```rust
enum Message {
    User(Vec<ContentBlock>),
    Assistant(Vec<ContentBlock>),
}

enum ContentBlock {
    Text(String),
    ToolUse { id: String, name: String, input: JsonValue },
    ToolResult { tool_use_id: String, content: Vec<ContentBlock>, is_error: bool },
}
```

---

## 五、MCP 协议栈

### Transport Trait

```rust
trait McpTransport {
    async fn send(&mut self, msg: JsonRpcRequest) -> Result<()>;
    async fn recv(&mut self) -> Result<JsonRpcMessage>;
}
```

### 四种传输实现

```
src/mcp/transport/
├── stdio.rs    # 子进程 stdin/stdout，行分隔 JSON-RPC 2.0
├── sse.rs      # HTTP GET 长连接（复用 net/sse.rs）
├── http.rs     # HTTP POST 单次请求响应
└── ws.rs       # WebSocket（复用 net/tls.rs + 手实现帧协议）
```

### Client 生命周期

```
connect() → initialize(capabilities) → list_tools() → list_resources()
  ↓
call_tool(name, args) → ToolResult
  ↓
close()
```

### 工具动态注册

MCP 工具自动转为内置 Tool，命名格式 `mcp__serverName__toolName`（与 Claude Code 一致），input schema 直接透传。

### 配置格式

```json
{
  "mcpServers": {
    "filesystem": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/path"]
    },
    "remote": {
      "type": "sse",
      "url": "https://mcp.example.com/sse"
    }
  }
}
```

---

## 六、Skill 系统

### 目录层级（优先级从高到低）

```
.claude/skills/                  # 项目级
~/.claude/skills/                # 用户级
~/.claude/plugins/cache/         # 插件缓存
```

### Skill 文件格式

```markdown
---
name: my-skill
description: 做某件事时使用
---

Skill 内容正文...
```

### 加载器

```rust
struct SkillLoader {
    search_paths: Vec<PathBuf>,
}

impl SkillLoader {
    async fn discover(&self) -> Vec<SkillMeta>;
    async fn load(&self, name: &str) -> SkillContent;
}
```

同名 skill 项目级覆盖用户级，用户级覆盖插件目录。

### Skill 执行

Skill 内容注入当前对话的 system prompt，由 Agent 决策如何使用。

### Hook 触发

```json
{
  "hooks": {
    "PreToolUse":   [{ "matcher": "Bash", "hooks": [{ "type": "skill", "skill": "security-check" }] }],
    "PostToolUse":  [...],
    "Stop":         [...],
    "Notification": [...]
  }
}
```

支持事件：`PreToolUse` / `PostToolUse` / `Stop` / `Notification`，与 Claude Code 完全一致。

---

## 七、权限模型

### 三种模式

| 模式 | 说明 |
|------|------|
| `default` | 按规则匹配，未匹配的危险操作询问用户 |
| `auto` | 自动批准大多数操作，仅最危险的询问 |
| `bypass` | 全自动，不询问 |

### 规则配置

```json
{
  "permissions": {
    "allow": ["Bash(git *)", "Read(*)", "Write(src/**)"],
    "deny":  ["Bash(rm -rf *)", "Write(/etc/**)"],
    "ask":   ["Bash(*)", "WebFetch(*)"]
  }
}
```

匹配优先级：`deny` > `allow` > `ask` > 模式默认行为。

### 执行流程

```
tool.call() 前：
  1. validateInput(schema)
  2. matchRules(allow/deny/ask)     → 直接允许 / 拒绝
  3. 未匹配 → AI 动态分类器（fast tier）
  4. 分类器判断危险 → TUI inline 询问用户
  5. 用户 y/n/Enter → 继续 / 中止
```

### AI 动态分类器

对未匹配规则的操作，用 fast tier 模型判断风险：`safe` / `caution` / `dangerous`，避免把每个边界情况硬编码进规则。

### UI 交互

权限询问在 TUI 中以 inline 提示展示，用户按 `y`/`n`/`Enter` 确认，与 Claude Code 确认框体验一致。

---

## 八、实现顺序建议

1. `src/runtime/` — 自制 async executor（基础，其他模块依赖）
2. `src/agent/` — Agent 循环 + 消息类型
3. `src/tools/` — Tool trait + 核心工具（Bash、文件、搜索）
4. `src/permissions/` — 权限规则 + AI 分类器
5. `src/mcp/` — MCP 协议栈（stdio 优先，再加 SSE/HTTP/WS）
6. `src/skills/` — Skill 加载 + Hook 系统
7. 补全剩余 60+ 工具
8. 集成测试（`--features full_test`）
