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

**核心原则：最大化自动审核，最小化用户打断。** 只有真正不可逆、高风险的操作才询问用户，其余全部自动放行。

### 三种模式

| 模式 | 说明 |
|------|------|
| `default` | 内置安全规则 + AI 分类器，绝大多数操作自动批准，仅不可逆高危操作询问 |
| `auto` | 在 default 基础上进一步放宽，AI 分类器阈值提高，几乎不询问 |
| `bypass` | 全自动，永不询问 |

### 内置默认允许规则（无需配置）

以下操作永远自动批准，不进 AI 分类器：

- 所有**只读**操作：Read、Glob、Grep、LS、WebFetch
- **写入项目目录**内的文件（相对路径 / cwd 子目录）
- **Git 操作**：`git status/log/diff/add/commit/push/pull` 等
- **包管理**：`cargo/npm/pip install` 等构建相关命令
- **MCP 工具调用**（已通过配置显式启用）

### 内置默认拒绝规则（硬性拦截）

以下操作永远拒绝，不询问：

- `rm -rf /`、`rm -rf ~`、`dd if=/dev/zero` 等明显破坏性命令
- 写入系统目录：`/etc/**`、`/usr/**`、`/bin/**`、`/sbin/**`
- `sudo` / `su` 提权命令

### 用户自定义规则（`.claude/settings.json`）

```json
{
  "permissions": {
    "allow": ["Bash(git *)", "Write(src/**)"],
    "deny":  ["Bash(curl * | bash)"],
    "ask":   ["Bash(npm publish *)"]
  }
}
```

匹配优先级：用户 `deny` > 用户 `allow` > 内置拒绝 > 内置允许 > AI 分类器。

### 执行流程（优化为最少询问）

```
tool.call() 前：
  1. validateInput(schema)
  2. 用户 deny 规则命中 → 直接拒绝
  3. 用户 allow 规则命中 → 直接执行
  4. 内置拒绝规则命中   → 直接拒绝
  5. 内置允许规则命中   → 直接执行
  6. 以上都未命中 → AI 动态分类器（fast tier，<100ms）
     - safe / caution → 自动执行（记录日志）
     - dangerous      → TUI inline 询问用户
  7. 用户 y/n/Enter → 继续 / 中止（并可选"记住这个规则"）
```

### AI 动态分类器（高自动化阈值）

- 使用 fast tier 模型，目标延迟 <100ms
- 分级：`safe`（直接执行）/ `caution`（执行但警告）/ `dangerous`（询问）
- **阈值偏宽松**：有歧义时倾向 `caution` 而非 `dangerous`，减少打断
- 分类结果可缓存：相同命令模式在同一会话内只分类一次
- 用户确认后可选"记住此规则"，自动写入 allow/deny 列表

### UI 交互

仅 `dangerous` 级别触发 TUI inline 询问，展示操作摘要和风险说明，用户按 `y`/`n`/`Enter` 确认。确认界面提供"本次允许" / "始终允许" / "拒绝"三个选项。

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
