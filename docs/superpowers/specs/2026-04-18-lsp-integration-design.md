# LSP Integration Design

## Overview

为 viv 添加内置 LSP (Language Server Protocol) 客户端，让 Agent 获得代码语义理解能力：跳转定义、查找引用、类型悬停、诊断信息。架构镜像现有 MCP 模块，复用 JSON-RPC 和 StdioTransport 基础设施。

## MVP 范围

**只读分析能力（4 个 Tool）：**
- `LspDefinition` — textDocument/definition
- `LspReferences` — textDocument/references
- `LspHover` — textDocument/hover
- `LspDiagnostics` — textDocument/publishDiagnostics（被动接收）+ 主动查询缓存

**不在 MVP 范围：**
- 写操作（code-actions、rename、formatting）
- 自动检测 LSP server
- Completion、signature-help、code-lens

## 架构

### 目录结构

```
src/lsp/
├── mod.rs              # LspManager — server 生命周期（懒加载 + 保活）
├── client.rs           # LspClient — JSON-RPC 请求/响应 + LSP 握手
├── types.rs            # LSP 类型（Position, Range, Location, Hover, Diagnostic 等）
├── config.rs           # .viv/settings.json → lspServers 解析
└── tools.rs            # 4 个 Tool 实现
```

### 数据流

```
Agent 调用 LspDefinition tool
  → tool 从 LspManager 获取/启动 server（按文件扩展名映射）
    → LspClient 发 JSON-RPC request（Content-Length 分帧）
      → LSP server 返回 response
    → LspClient 解析结果
  → tool 格式化为 Agent 可读的文本（含文件路径、行号、代码片段）
Agent 拿到结果继续推理
```

### 与 MCP 的关系

完全独立的模块，不修改 MCP 代码。共用：
- `src/core/jsonrpc.rs` — JSON-RPC 2.0 消息类型
- `src/mcp/transport/stdio.rs` — StdioTransport（增加 Content-Length 分帧模式）

## 详细设计

### 1. Transport 层适配

给现有 `StdioTransport` 增加分帧模式：

```rust
pub enum Framing {
    Newline,         // MCP: 一行一条消息
    ContentLength,   // LSP: Content-Length: N\r\n\r\n{body}
}

pub struct StdioTransport {
    child: Child,
    stdin_fd: RawFd,
    stdout_fd: RawFd,
    read_buf: Vec<u8>,
    framing: Framing,    // 新增
}
```

- `send()`: `Newline` 模式追加 `\n`；`ContentLength` 模式写 `Content-Length: {len}\r\n\r\n{body}`
- `recv()`: `Newline` 模式按 `\n` 分割；`ContentLength` 模式先解析 header 得到 length，再读取 body
- MCP 调用不受影响（`spawn()` 默认 `Newline`，新增 `spawn_with_framing()` 或参数）

### 2. LspClient

```rust
pub struct LspClient {
    transport: StdioTransport,
    next_id: i64,
    server_capabilities: Option<ServerCapabilities>,
    opened_files: HashSet<PathBuf>,        // 已发送 didOpen 的文件
    diagnostics: HashMap<PathBuf, Vec<Diagnostic>>,  // 缓存 server 推送的诊断
}
```

**初始化握手（LSP 协议要求）：**
1. Client → `initialize` request
   - 发送 `clientCapabilities`（声明支持 definition/references/hover/diagnostics）
   - 发送 `rootUri`（当前工作目录）
2. Server → 返回 `ServerCapabilities`（告知支持哪些功能）
3. Client → `initialized` notification（握手完成）

**核心方法：**
```rust
impl LspClient {
    pub async fn initialize(&mut self, root_uri: &str) -> Result<()>;
    pub async fn definition(&mut self, file: &Path, line: u32, col: u32) -> Result<Vec<Location>>;
    pub async fn references(&mut self, file: &Path, line: u32, col: u32) -> Result<Vec<Location>>;
    pub async fn hover(&mut self, file: &Path, line: u32, col: u32) -> Result<Option<HoverResult>>;
    pub fn cached_diagnostics(&self, file: &Path) -> Vec<Diagnostic>;

    // 内部：确保文件已 didOpen
    async fn ensure_open(&mut self, file: &Path) -> Result<()>;
    // 内部：处理 server 推送的 notification
    fn handle_notification(&mut self, notification: &Notification);
}
```

**文件同步：** 调用任何 LSP 方法时，自动检查文件是否已 `didOpen`。若未打开，先读取文件内容并发送 `textDocument/didOpen` 通知。用 `opened_files: HashSet<PathBuf>` 跟踪状态。

**诊断缓存：** LSP server 主动推送 `textDocument/publishDiagnostics` notification。在 `recv()` 循环中拦截并缓存到 `diagnostics` 字段。`LspDiagnostics` tool 读取缓存。

### 3. LspManager

```rust
pub struct LspManager {
    config: LspConfig,
    servers: HashMap<String, Option<LspClient>>,  // name → None=未启动, Some=已连接
}

impl LspManager {
    pub fn from_config(config: LspConfig) -> Self;

    /// 按文件扩展名找到 server，首次调用时启动
    pub async fn get_or_start(&mut self, file: &Path) -> Result<&mut LspClient>;

    /// 按 server 名获取（内部用）
    async fn start_server(&mut self, name: &str) -> Result<()>;

    /// 会话结束时 shutdown 所有已启动的 server
    pub async fn shutdown_all(&mut self);

    /// 文件扩展名 → server 名映射
    fn server_for_file(&self, file: &Path) -> Option<&str>;
}
```

**生命周期：** 懒加载 + 保活。Agent 启动时只加载配置，不启动任何 server。首次调用 LSP tool 时启动对应 server。一旦启动，保持到会话结束。`Agent::shutdown()` 中调用 `lsp_manager.shutdown_all()`。

### 4. 四个 Tool

所有 Tool 权限等级为 `ReadOnly`。

#### LspDefinition

```
输入 schema: { "file": string, "line": integer, "column": integer }
LSP method: textDocument/definition
输出格式:
  "定义位置: src/agent/agent.rs:133:5
   pub async fn run(&mut self) -> Result<()> {"
```

Tool 拿到 `Location` 后，用 `FileRead` 读取目标文件对应行附近的代码片段（前后 2 行），拼接到输出中，让 Agent 直接看到定义内容。

#### LspReferences

```
输入 schema: { "file": string, "line": integer, "column": integer }
LSP method: textDocument/references
输出格式:
  "找到 6 个引用:
   src/agent/agent.rs:45:9 — self.tools.register(Box::new(bash))
   src/agent/agent.rs:98:20 — self.tools.get(name)
   ..."
```

每个引用附带该行代码内容。引用数量超过 20 个时截断并提示总数。

#### LspHover

```
输入 schema: { "file": string, "line": integer, "column": integer }
LSP method: textDocument/hover
输出格式:
  "类型: HashMap<String, Option<LspClient>>
   文档: Standard hashmap from std::collections"
```

返回类型签名和文档字符串（如果有）。

#### LspDiagnostics

```
输入 schema: { "file": string (可选) }
LSP method: 读取缓存的 publishDiagnostics
输出格式:
  "src/main.rs:
     12:5 error[E0599]: unused variable `x`
     30:1 warning: missing docs
   src/lsp/client.rs:
     45:10 error[E0308]: mismatched types"
```

指定 file 时只返回该文件的诊断；不指定时返回所有已缓存诊断。

### 5. 配置

**`.viv/settings.json` 扩展：**
```json
{
  "mcpServers": { ... },
  "lspServers": {
    "rust": {
      "command": "rust-analyzer",
      "args": [],
      "extensions": [".rs"]
    },
    "python": {
      "command": "pylsp",
      "args": [],
      "extensions": [".py"]
    }
  }
}
```

- `command`: LSP server 可执行文件
- `args`: 启动参数
- `extensions`: 该 server 处理的文件扩展名列表
- 可选 `env`: 环境变量（同 MCP 格式）

### 6. Error 枚举扩展

```rust
pub enum Error {
    // ... 现有变体
    Lsp { server: String, message: String },
}
```

### 7. 容错策略

| 场景 | 处理 |
|------|------|
| 未配置对应语言的 LSP server | Tool 返回提示信息，不中断 Agent |
| Server 启动失败 | 返回错误，Agent 可 fallback 到 Grep/FileRead |
| Server 运行中崩溃 | 标记为 None，下次调用重新启动 |
| 请求超时 | 10 秒超时，返回超时错误 |
| Server 不支持某个能力 | 检查 ServerCapabilities，不支持则返回提示 |

## Agent 集成

### Tool 注册

在 `Agent` 初始化时，创建 `LspManager`（只加载配置），注册 4 个 LSP tool 到 `ToolRegistry`。tool 持有 `Arc<Mutex<LspManager>>` 引用。

### shutdown 流程

`Agent::shutdown()` 增加 `lsp_manager.shutdown_all()` 调用，发送 `shutdown` + `exit` 给所有已启动的 LSP server。

## 未来扩展（不在 MVP）

- **自动检测**：扫描项目文件，自动匹配并启动 LSP server
- **写操作**：code-actions、rename、formatting
- **didChange 同步**：Agent 编辑文件后通知 LSP server 更新
- **Completion**：代码补全（可能不需要，Agent 自己生成代码）

## 测试策略

- **单元测试**：Content-Length 分帧的 encode/decode、LSP 类型解析、配置解析
- **集成测试**：mock LSP server（简单的 stdio 程序，返回预设响应）测试完整流程
- **e2e 测试**（`--features full_test`）：连接真实 rust-analyzer，测试 definition/references/hover
