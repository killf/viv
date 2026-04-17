# viv 系统异步化 + MCP 协议栈设计文档

**日期：** 2026-04-17
**状态：** 待审核
**范围：** 全系统异步化 + MCP（tools / resources / prompts）四种传输

---

## 一、目标

1. **全系统异步化**：将 Agent 线程从同步阻塞改为异步，基于现有自制 async runtime（executor + reactor + epoll）
2. **MCP 协议栈**：实现 MCP 客户端，支持 stdio / SSE / HTTP / WebSocket 四种传输，完整支持 tools / resources / prompts 三大能力

**核心约束：** 零外部依赖（edition 2024 单 crate）。

---

## 二、整体架构

### 双线程模型

```
┌─────────────────────────────────────────────────────────┐
│                     viv Runtime                          │
│                                                          │
│  ┌──────────────┐   mpsc channel   ┌──────────────────┐ │
│  │  UI Thread   │◄────────────────►│  Runtime Thread  │ │
│  │              │                  │                  │ │
│  │  epoll       │  AgentEvent ──►  │  Executor        │ │
│  │  + render    │  ◄── AgentMsg    │  + Reactor       │ │
│  │  (同步，不变)  │                  │                  │ │
│  └──────────────┘                  │  ┌────────────┐  │ │
│                                    │  │ Agent Loop │  │ │
│                                    │  │  (Future)  │  │ │
│                                    │  └─────┬──────┘  │ │
│                                    │        │         │ │
│                                    │  ┌─────▼──────┐  │ │
│                                    │  │ LLM stream │  │ │
│                                    │  │ Tool tasks │  │ │
│                                    │  │ MCP calls  │  │ │
│                                    │  └────────────┘  │ │
│                                    └──────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

UI Thread 保持同步 epoll 驱动，不变。Agent 侧全部异步化，运行在 Runtime Thread 的 executor 上。两线程通过 channel 通信。

### 关键变化

| 组件 | 现在（同步） | 目标（异步） |
|------|------------|------------|
| `Agent::run()` | `loop { rx.recv() }` 阻塞 | `async fn run()` 在 executor 上 |
| `LlmClient::stream_agent()` | TLS 阻塞读写 | `AsyncTlsStream` 非阻塞 |
| `Tool::execute()` | `fn execute() -> Result<String>` | `async fn execute()` |
| channel 接收 | `mpsc::Receiver::recv()` 阻塞 | `AsyncReceiver` 注册到 reactor |
| MCP | 不存在 | 原生 async |

### 新增/修改模块

```
src/core/
├── jsonrpc.rs              # 新增：JSON-RPC 2.0 通用实现
├── net/
│   ├── async_tls.rs        # 新增：AsyncTlsStream（异步 TLS 握手+读写）
│   └── ws.rs               # 新增：WebSocket 帧协议
├── runtime/
│   └── channel.rs          # 新增：async channel（UI→Agent 异步接收）

src/mcp/
├── mod.rs                  # McpManager 公开 API
├── client.rs               # McpClient<T: Transport>
├── types.rs                # MCP 数据类型（Tool, Resource, Prompt 等）
├── config.rs               # 配置加载（.viv/settings.json）
└── transport/
    ├── mod.rs              # Transport trait (async)
    ├── stdio.rs            # stdio 传输（子进程管理）
    ├── sse.rs              # SSE 传输
    ├── http.rs             # HTTP 传输（Streamable HTTP）
    └── ws.rs               # WebSocket 传输

src/llm.rs                  # 修改：stream_agent() → async
src/tools/mod.rs            # 修改：Tool::execute() → async
src/agent/agent.rs          # 修改：Agent::run() → async
src/main.rs                 # 修改：Runtime 线程启动 Agent Future
```

---

## 三、异步基础设施补全

### 3.1 AsyncTlsStream（`src/core/net/async_tls.rs`）

在 `AsyncTcpStream` 之上包装 OpenSSL TLS，握手和读写全部异步。

```rust
pub struct AsyncTlsStream {
    tcp: AsyncTcpStream,
    ssl: *mut SSL,
}

impl AsyncTlsStream {
    pub async fn connect(host: &str, port: u16) -> Result<Self>;
    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
    pub async fn write_all(&mut self, buf: &[u8]) -> Result<()>;
}
```

**异步 TLS 握手：**

TCP socket 设为非阻塞模式（`AsyncTcpStream` 已做）。OpenSSL 在非阻塞 socket 上的 `SSL_connect()` 会返回 `SSL_ERROR_WANT_READ` / `SSL_ERROR_WANT_WRITE`，此时注册 fd 到 reactor 等待就绪后重试：

```rust
async fn async_handshake(&mut self) -> Result<()> {
    loop {
        let ret = unsafe { SSL_connect(self.ssl) };
        if ret == 1 { return Ok(()); }
        match ssl_get_error(self.ssl, ret) {
            SSL_ERROR_WANT_READ  => reactor_wait_readable(self.tcp.raw_fd()).await?,
            SSL_ERROR_WANT_WRITE => reactor_wait_writable(self.tcp.raw_fd()).await?,
            err => return Err(Error::Tls(ssl_error_string(err))),
        }
    }
}
```

**异步读写：** 同理，`SSL_read` / `SSL_write` 返回 `WANT_READ` / `WANT_WRITE` 时注册 reactor 等待。

**辅助函数：**

```rust
async fn reactor_wait_readable(fd: RawFd) -> Result<()>;
async fn reactor_wait_writable(fd: RawFd) -> Result<()>;
```

内部创建一次性 Future，注册到全局 reactor，唤醒后 resolve。

### 3.2 Async Channel（`src/core/runtime/channel.rs`）

Agent 侧需要异步接收 UI 事件。基于 pipe + reactor 实现：

```rust
pub struct AsyncReceiver<T> {
    rx: mpsc::Receiver<T>,
    pipe_read: RawFd,
}

pub struct NotifySender<T> {
    tx: mpsc::Sender<T>,
    pipe_write: RawFd,
}
```

**原理：**
- `NotifySender::send(msg)` → `tx.send(msg)` + 往 pipe 写 1 字节唤醒 reactor
- `AsyncReceiver::recv()` → `try_recv()`，如 Empty 则注册 `pipe_read` 到 reactor 等待

UI 线程用 `NotifySender`（同步 send），Agent 侧用 `AsyncReceiver`（async recv），零侵入改 UI 线程。

### 3.3 WebSocket 帧协议（`src/core/net/ws.rs`）

```rust
pub struct WsFrame {
    pub opcode: WsOpcode,
    pub payload: Vec<u8>,
}

pub enum WsOpcode { Text, Binary, Close, Ping, Pong }

pub async fn ws_handshake(stream: &mut AsyncTlsStream, host: &str, path: &str) -> Result<()>;
pub async fn ws_read_frame(stream: &mut AsyncTlsStream) -> Result<WsFrame>;
pub async fn ws_write_frame(stream: &mut AsyncTlsStream, frame: &WsFrame) -> Result<()>;
```

实现要点：
- 客户端帧必须 masking（RFC 6455）
- 2-14 字节帧头（根据 payload 长度变化）
- 4 字节 mask key，XOR payload
- 自动回复 Ping → Pong

### 3.4 JSON-RPC 2.0（`src/core/jsonrpc.rs`）

基于已有 `JsonValue` 构建：

```rust
pub struct Request {
    pub id: i64,
    pub method: String,
    pub params: Option<JsonValue>,
}

pub enum Response {
    Result { id: i64, result: JsonValue },
    Error { id: i64, error: RpcError },
}

pub struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<JsonValue>,
}

pub struct Notification {
    pub method: String,
    pub params: Option<JsonValue>,
}

pub enum Message {
    Request(Request),
    Response(Response),
    Notification(Notification),
}

impl Message {
    pub fn parse(json: &JsonValue) -> Result<Self>;
}

impl Request {
    pub fn to_json(&self) -> JsonValue;
}

impl Notification {
    pub fn to_json(&self) -> JsonValue;
}
```

每条消息自动加 `"jsonrpc": "2.0"` 字段。`id` 用 `i64`，客户端侧自增分配。

---

## 四、LLM 客户端异步化

### stream_agent() 改造

核心流程不变，I/O 换成 async：

```rust
impl LlmClient {
    pub async fn stream_agent(
        &self,
        system_blocks: &[SystemBlock],
        messages: &[Message],
        tools_json: &str,
        tier: ModelTier,
        on_text: impl Fn(&str),
    ) -> Result<StreamResult> {
        // 1. 构建 HTTP 请求（纯计算，不变）
        let request = self.build_request(system_blocks, messages, tools_json, tier);

        // 2. 异步连接
        let mut stream = AsyncTlsStream::connect(&self.config.base_url, 443).await?;
        stream.write_all(&request.to_bytes()).await?;

        // 3. 异步读响应头
        let response_header = read_http_header(&mut stream).await?;

        // 4. SSE 流式读取（复用 SseParser 的 feed/drain 模式）
        let mut parser = SseParser::new();
        let mut result = StreamResult::default();
        let mut buf = [0u8; 4096];

        loop {
            let n = stream.read(&mut buf).await?;
            if n == 0 { break; }
            parser.feed(&buf[..n]);

            while let Some(event) = parser.drain() {
                match event.event.as_str() {
                    "content_block_delta" => { /* 解析 delta，on_text 回调 */ }
                    "message_stop" => return Ok(result),
                    // ... 其他事件（不变）
                }
            }
        }
        Ok(result)
    }
}
```

`SseParser` 本身不需修改——其 `feed(bytes)` + `drain()` 模式天然适配异步循环。

---

## 五、Tool trait 异步化

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> JsonValue;
    fn permission_level(&self) -> PermissionLevel;

    async fn execute(&self, input: &JsonValue) -> Result<String>;
}
```

现有内置工具改造量小：
- **BashTool**：子进程 stdout fd 注册到 reactor 做非阻塞等待
- **文件工具**（Read/Write/Edit/Glob/Grep）：本地文件 I/O 足够快，直接同步操作在 async fn 中可接受
- **WebFetch**：改用 `AsyncTlsStream`，真正获益
- **MCP 工具代理**：天然 async

---

## 六、Agent 循环异步化

```rust
impl Agent {
    pub async fn run(mut self) -> Result<()> {
        loop {
            match self.event_rx.recv().await {
                AgentEvent::Input(text) => self.handle_input(text).await?,
                AgentEvent::Quit => {
                    self.evolve().await?;
                    self.mcp_manager.shutdown_all().await?;
                    break;
                }
                AgentEvent::Interrupt => { /* ... */ }
                AgentEvent::PermissionResponse(allowed) => { /* ... */ }
            }
        }
        Ok(())
    }

    async fn handle_input(&mut self, text: String) -> Result<()> {
        let memories = retrieve_relevant(&text, &self.index)?;
        let system = build_system_prompt(&memories, &mut self.prompt_cache);
        self.messages.push(Message::user_text(text));
        compact_if_needed(&mut self.messages)?;
        self.agentic_loop(system).await
    }

    async fn agentic_loop(&mut self, system: SystemPrompt) -> Result<()> {
        let tools_json = self.tools.to_api_json();

        for _ in 0..self.config.max_iterations {
            let stream_result = self.llm.stream_agent(
                &system.blocks, &self.messages, &tools_json,
                self.config.model_tier.clone(),
                |chunk| { self.msg_tx.send(AgentMessage::TextChunk(chunk.into())); },
            ).await?;

            self.messages.push(Message::assistant(stream_result.content_blocks()));

            if stream_result.tool_uses.is_empty() { break; }

            let mut tool_results = Vec::new();
            for tu in &stream_result.tool_uses {
                self.msg_tx.send(AgentMessage::ToolStart { name: tu.name.clone(), input: tu.input.to_string() });
                let result = self.execute_tool(tu).await;
                self.msg_tx.send(AgentMessage::ToolEnd { name: tu.name.clone(), output: result.summary() });
                tool_results.push(result);
            }

            self.messages.push(Message::user(tool_results));
        }
        Ok(())
    }

    async fn execute_tool(&mut self, tu: &ToolUse) -> ContentBlock {
        let allowed = self.check_permission(&tu.name, &tu.input).await;
        let output = if allowed {
            self.tools.get(&tu.name).unwrap().execute(&tu.input).await
        } else {
            Err(Error::Tool("permission denied".into()))
        };
        ContentBlock::ToolResult {
            tool_use_id: tu.id.clone(),
            content: match &output {
                Ok(text) => vec![ContentBlock::Text(text.clone())],
                Err(e) => vec![ContentBlock::Text(e.to_string())],
            },
            is_error: output.is_err(),
        }
    }
}
```

### main.rs 入口

```rust
fn main() -> Result<()> {
    let (event_tx, event_rx) = async_channel();    // NotifySender + AsyncReceiver
    let (msg_tx, msg_rx) = std::sync::mpsc::channel();

    // Agent 在 runtime 线程上异步运行
    let runtime = Runtime::new();
    runtime.spawn(|executor| {
        executor.spawn(async move {
            let agent = Agent::new(event_rx, msg_tx).await.unwrap();
            agent.run().await.unwrap();
        });
    });

    // UI 线程同步运行（不变）
    let mut ui = TerminalUI::new(event_tx, msg_rx)?;
    ui.run()
}
```

---

## 七、MCP 协议栈

### 7.1 Transport Trait

```rust
pub trait Transport {
    async fn send(&mut self, msg: JsonValue) -> Result<()>;
    async fn recv(&mut self) -> Result<JsonValue>;
    async fn close(&mut self) -> Result<()>;
}
```

### 7.2 stdio 传输（`src/mcp/transport/stdio.rs`）

```rust
pub struct StdioTransport {
    child: Child,
    stdin: ChildStdin,
    stdout_fd: RawFd,       // 非阻塞，注册到 reactor
    read_buf: Vec<u8>,
}

impl StdioTransport {
    pub fn spawn(command: &str, args: &[String], env: &[(String, String)]) -> Result<Self> {
        // Command::new(command).args(args).envs(env)
        // .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        // .spawn()
        // stdout fd 设为非阻塞
    }
}

impl Transport for StdioTransport {
    async fn send(&mut self, msg: JsonValue) -> Result<()> {
        let line = format!("{}\n", msg);
        self.stdin.write_all(line.as_bytes())?;
        self.stdin.flush()?;
        Ok(())
    }

    async fn recv(&mut self) -> Result<JsonValue> {
        loop {
            if let Some(pos) = self.read_buf.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = self.read_buf.drain(..=pos).collect();
                return JsonValue::parse(std::str::from_utf8(&line)?.trim());
            }
            reactor_wait_readable(self.stdout_fd).await?;
            let n = nonblocking_read(self.stdout_fd, &mut buf)?;
            if n == 0 { return Err(Error::Mcp { server: String::new(), message: "server closed".into() }); }
            self.read_buf.extend_from_slice(&buf[..n]);
        }
    }

    async fn close(&mut self) -> Result<()> {
        // 关闭 stdin → 等待子进程退出（超时 5 秒后 kill）
    }
}
```

### 7.3 SSE 传输（`src/mcp/transport/sse.rs`）

```rust
pub struct SseTransport {
    read_stream: AsyncTlsStream,
    post_url: String,
    post_host: String,
    sse_parser: SseParser,
}

impl SseTransport {
    pub async fn connect(url: &str) -> Result<Self> {
        // 1. 解析 URL
        // 2. AsyncTlsStream::connect()
        // 3. HTTP GET 建立 SSE 连接
        // 4. 读取 endpoint 事件获取 POST URL
    }
}

impl Transport for SseTransport {
    async fn send(&mut self, msg: JsonValue) -> Result<()> {
        // 新建短连接 POST 到 endpoint URL
    }

    async fn recv(&mut self) -> Result<JsonValue> {
        // 从 SSE 流读取下一个 message 事件
        loop {
            let event = read_sse_event(&mut self.read_stream, &mut self.sse_parser).await?;
            if event.event == "message" {
                return JsonValue::parse(&event.data);
            }
        }
    }

    async fn close(&mut self) -> Result<()> { Ok(()) }
}
```

### 7.4 HTTP 传输（`src/mcp/transport/http.rs`）

HTTP 传输是请求-响应模式：每次 `send` 立即 POST 并将响应存入缓冲，`recv` 从缓冲取出。这保证了 McpClient 的 `request()` 方法（先 send 再 recv）正确工作，且不会因连续 send 丢消息。

```rust
pub struct HttpTransport {
    base_url: String,
    host: String,
    port: u16,
    path: String,
    session_id: Option<String>,
    response_buf: VecDeque<JsonValue>,
}

impl Transport for HttpTransport {
    async fn send(&mut self, msg: JsonValue) -> Result<()> {
        let mut stream = AsyncTlsStream::connect(&self.host, self.port).await?;
        let mut headers = vec![("Content-Type", "application/json")];
        if let Some(sid) = &self.session_id {
            headers.push(("Mcp-Session-Id", sid));
        }
        let req = HttpRequest::post(&self.host, &self.path, &headers, &msg.to_string());
        stream.write_all(&req.to_bytes()).await?;
        let resp = read_http_response(&mut stream).await?;
        if let Some(sid) = resp.header("mcp-session-id") {
            self.session_id = Some(sid.to_string());
        }
        let body = JsonValue::parse(&resp.body)?;
        self.response_buf.push_back(body);
        Ok(())
    }

    async fn recv(&mut self) -> Result<JsonValue> {
        self.response_buf.pop_front()
            .ok_or(Error::Mcp { server: String::new(), message: "no response available".into() })
    }

    async fn close(&mut self) -> Result<()> { Ok(()) }
}
```

### 7.5 WebSocket 传输（`src/mcp/transport/ws.rs`）

```rust
pub struct WsTransport {
    stream: AsyncTlsStream,
}

impl WsTransport {
    pub async fn connect(url: &str) -> Result<Self> {
        // 解析 wss:// URL → AsyncTlsStream::connect() → ws_handshake()
    }
}

impl Transport for WsTransport {
    async fn send(&mut self, msg: JsonValue) -> Result<()> {
        ws_write_frame(&mut self.stream, &WsFrame::text(&msg.to_string())).await
    }

    async fn recv(&mut self) -> Result<JsonValue> {
        loop {
            let frame = ws_read_frame(&mut self.stream).await?;
            match frame.opcode {
                WsOpcode::Text => return JsonValue::parse(&String::from_utf8(frame.payload)?),
                WsOpcode::Ping => ws_write_frame(&mut self.stream, &WsFrame::pong(&frame.payload)).await?,
                WsOpcode::Close => return Err(Error::Mcp { .. }),
                _ => {}
            }
        }
    }

    async fn close(&mut self) -> Result<()> {
        ws_write_frame(&mut self.stream, &WsFrame::close()).await
    }
}
```

### 7.6 McpClient

```rust
pub struct McpClient<T: Transport> {
    transport: T,
    next_id: i64,
    server_capabilities: Option<ServerCapabilities>,
    pending_notifications: Vec<Notification>,
}

impl<T: Transport> McpClient<T> {
    /// 底层：发请求等响应，中间拦截 notification
    async fn request(&mut self, method: &str, params: Option<JsonValue>) -> Result<JsonValue> {
        let id = self.next_id;
        self.next_id += 1;
        let req = Request { id, method: method.into(), params };
        self.transport.send(req.to_json()).await?;

        loop {
            let msg = self.transport.recv().await?;
            match Message::parse(&msg)? {
                Message::Response(Response::Result { id: rid, result }) if rid == id => return Ok(result),
                Message::Response(Response::Error { id: rid, error }) if rid == id => {
                    return Err(Error::JsonRpc { code: error.code, message: error.message });
                }
                Message::Notification(n) => self.pending_notifications.push(n),
                _ => {}
            }
        }
    }

    async fn notify(&mut self, method: &str, params: Option<JsonValue>) -> Result<()> {
        let notif = Notification { method: method.into(), params };
        self.transport.send(notif.to_json()).await
    }

    // --- 生命周期 ---

    pub async fn initialize(&mut self) -> Result<ServerCapabilities> {
        let params = /* { protocolVersion, capabilities, clientInfo } */;
        let result = self.request("initialize", Some(params)).await?;
        let caps = ServerCapabilities::from_json(&result)?;
        self.server_capabilities = Some(caps.clone());
        self.notify("notifications/initialized", None).await?;
        Ok(caps)
    }

    pub async fn shutdown(&mut self) -> Result<()> {
        self.transport.close().await
    }

    // --- Tools ---

    pub async fn list_tools(&mut self) -> Result<Vec<McpTool>> {
        // 支持 pagination（cursor）
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let params = cursor.map(|c| json_object! { "cursor" => c });
            let result = self.request("tools/list", params).await?;
            // 解析 tools 数组，追加到 all
            match result.get("nextCursor").and_then(|v| v.as_str()) {
                Some(c) => cursor = Some(c.to_string()),
                None => break,
            }
        }
        Ok(all)
    }

    pub async fn call_tool(&mut self, name: &str, args: &JsonValue) -> Result<ToolCallResult> {
        let params = json_object! { "name" => name, "arguments" => args.clone() };
        let result = self.request("tools/call", Some(params)).await?;
        ToolCallResult::from_json(&result)
    }

    // --- Resources ---

    pub async fn list_resources(&mut self) -> Result<Vec<McpResource>> {
        let result = self.request("resources/list", None).await?;
        McpResource::parse_list(&result)
    }

    pub async fn read_resource(&mut self, uri: &str) -> Result<ResourceContent> {
        let params = json_object! { "uri" => uri };
        let result = self.request("resources/read", Some(params)).await?;
        ResourceContent::from_json(&result)
    }

    // --- Prompts ---

    pub async fn list_prompts(&mut self) -> Result<Vec<McpPrompt>> {
        let result = self.request("prompts/list", None).await?;
        McpPrompt::parse_list(&result)
    }

    pub async fn get_prompt(&mut self, name: &str, args: &JsonValue) -> Result<PromptMessages> {
        let params = json_object! { "name" => name, "arguments" => args.clone() };
        let result = self.request("prompts/get", Some(params)).await?;
        PromptMessages::from_json(&result)
    }
}
```

### 7.7 MCP 数据类型（`types.rs`）

```rust
pub struct ServerCapabilities {
    pub tools: Option<ToolsCapability>,         // { listChanged: bool }
    pub resources: Option<ResourcesCapability>, // { subscribe: bool, listChanged: bool }
    pub prompts: Option<PromptsCapability>,     // { listChanged: bool }
}

pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: JsonValue,
}

pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

pub struct McpPrompt {
    pub name: String,
    pub description: Option<String>,
    pub arguments: Vec<PromptArgument>,
}

pub struct PromptArgument {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
}

pub struct ToolCallResult {
    pub content: Vec<ContentItem>,
    pub is_error: bool,
}

pub enum ContentItem {
    Text(String),
    Image { data: String, mime_type: String },
    Resource { uri: String, text: String },
}

pub struct ResourceContent {
    pub contents: Vec<ResourceContentItem>,
}

pub enum ResourceContentItem {
    Text { uri: String, mime_type: Option<String>, text: String },
    Blob { uri: String, mime_type: Option<String>, blob: String },
}

pub struct PromptMessages {
    pub description: Option<String>,
    pub messages: Vec<PromptMessage>,
}

pub struct PromptMessage {
    pub role: String,     // "user" / "assistant"
    pub content: ContentItem,
}
```

---

## 八、McpManager 与 Agent 集成

### McpManager

```rust
pub struct McpManager {
    servers: Vec<McpServerHandle>,
}

pub struct McpServerHandle {
    pub name: String,
    pub tools: Vec<McpTool>,
    pub resources: Vec<McpResource>,
    pub prompts: Vec<McpPrompt>,
    client: McpClientKind,
}

pub enum McpClientKind {
    Stdio(McpClient<StdioTransport>),
    Sse(McpClient<SseTransport>),
    Http(McpClient<HttpTransport>),
    Ws(McpClient<WsTransport>),
}
```

`McpClientKind` 提供统一的 async 方法（通过 match 分发到具体 transport）：

```rust
impl McpClientKind {
    async fn initialize(&mut self) -> Result<ServerCapabilities>;
    async fn call_tool(&mut self, name: &str, args: &JsonValue) -> Result<ToolCallResult>;
    async fn read_resource(&mut self, uri: &str) -> Result<ResourceContent>;
    async fn get_prompt(&mut self, name: &str, args: &JsonValue) -> Result<PromptMessages>;
    async fn list_tools(&mut self) -> Result<Vec<McpTool>>;
    async fn list_resources(&mut self) -> Result<Vec<McpResource>>;
    async fn list_prompts(&mut self) -> Result<Vec<McpPrompt>>;
    async fn close(&mut self) -> Result<()>;
}
```

### McpManager API

```rust
impl McpManager {
    pub async fn from_config(config: &McpConfig) -> Result<Self> {
        // 遍历 config.servers，逐个 connect + initialize + list_*
        // 单个服务器失败 → 跳过，警告日志
    }

    pub async fn call_tool(&mut self, server: &str, tool: &str, args: &JsonValue) -> Result<String>;
    pub async fn read_resource(&mut self, server: &str, uri: &str) -> Result<ResourceContent>;
    pub async fn get_prompt(&mut self, server: &str, name: &str, args: &JsonValue) -> Result<PromptMessages>;
    pub async fn shutdown_all(&mut self) -> Result<()>;
}
```

### Agent 集成

MCP 在 Agent 初始化时启动，通过三种方式集成：

**1. MCP Tools → ToolRegistry 动态注册**

每个 MCP 工具创建一个 `McpToolProxy` 包装，实现 `Tool` trait：

```rust
pub struct McpToolProxy {
    full_name: String,          // "mcp__filesystem__read_file"
    server_name: String,
    tool_name: String,
    description: String,
    schema: JsonValue,
    manager: Arc<Mutex<McpManager>>,
}

impl Tool for McpToolProxy {
    fn name(&self) -> &str { &self.full_name }
    fn description(&self) -> &str { &self.description }
    fn input_schema(&self) -> JsonValue { self.schema.clone() }
    fn permission_level(&self) -> PermissionLevel { PermissionLevel::Execute }

    async fn execute(&self, input: &JsonValue) -> Result<String> {
        let mut mgr = self.manager.lock().unwrap();
        mgr.call_tool(&self.server_name, &self.tool_name, input).await
    }
}
```

命名格式：`mcp__{serverName}__{toolName}`（与 Claude Code 一致）。

**2. MCP Resources → 内置工具**

提供两个内置工具让 Agent 发现和读取 MCP 资源：

- `ListMcpResources`：列出所有 MCP 服务器的可用资源
- `ReadMcpResource`：按 server + URI 读取资源内容

**3. MCP Prompts → 内置工具**

- `ListMcpPrompts`：列出所有可用 prompt 模板
- `GetMcpPrompt`：获取指定 prompt 的消息内容

### Agent 初始化流程

```rust
impl Agent {
    pub async fn new(event_rx: AsyncReceiver<AgentEvent>, msg_tx: Sender<AgentMessage>) -> Result<Self> {
        let llm = LlmClient::new(LlmConfig::from_env()?);

        // MCP 启动
        let mcp_config = McpConfig::load(".viv/settings.json")?;
        let mcp_manager = McpManager::from_config(&mcp_config).await?;
        let mcp = Arc::new(Mutex::new(mcp_manager));

        // 注册工具：内置 + MCP tools + MCP resource/prompt 工具
        let mut tools = ToolRegistry::default_tools();
        for handle in &mcp.lock().unwrap().servers {
            for tool in &handle.tools {
                tools.register(Box::new(McpToolProxy::new(handle, tool, mcp.clone())));
            }
        }
        tools.register(Box::new(ListMcpResourcesTool::new(mcp.clone())));
        tools.register(Box::new(ReadMcpResourceTool::new(mcp.clone())));
        tools.register(Box::new(ListMcpPromptsTool::new(mcp.clone())));
        tools.register(Box::new(GetMcpPromptTool::new(mcp.clone())));

        Ok(Self { llm, tools, mcp, event_rx, msg_tx, .. })
    }
}
```

---

## 九、配置

### `.viv/settings.json` 格式

```json
{
  "mcpServers": {
    "filesystem": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/home/user/projects"],
      "env": { "NODE_ENV": "production" }
    },
    "github": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": { "GITHUB_TOKEN": "ghp_xxx" }
    },
    "remote-api": {
      "type": "sse",
      "url": "https://mcp.example.com/sse"
    },
    "streamable": {
      "type": "http",
      "url": "https://mcp.example.com/mcp"
    },
    "ws-server": {
      "type": "websocket",
      "url": "wss://mcp.example.com/ws"
    }
  }
}
```

### 配置加载

```rust
pub struct McpConfig {
    pub servers: Vec<(String, ServerConfig)>,
}

pub enum ServerConfig {
    Stdio { command: String, args: Vec<String>, env: Vec<(String, String)> },
    Sse { url: String },
    Http { url: String },
    WebSocket { url: String },
}

impl McpConfig {
    pub fn load(path: &str) -> Result<Self>;
}
```

若 `.viv/settings.json` 不存在或无 `mcpServers` 字段，返回空配置，不报错。

---

## 十、错误处理

### 新增 Error 变体

```rust
pub enum Error {
    // 现有变体 ...
    JsonRpc { code: i64, message: String },       // JSON-RPC 协议错误
    Mcp { server: String, message: String },      // MCP 运行时错误
}
```

### 容错策略

- 单个 MCP 服务器连接失败 → 跳过，警告日志，其他服务器正常
- 工具调用时服务器断连 → 返回 `ToolResult { is_error: true }` 给 LLM
- 不因 MCP 失败中断 Agent

---

## 十一、实现顺序

1. **异步基础设施**
   - `src/core/net/async_tls.rs` — AsyncTlsStream（异步握手+读写）
   - `src/core/runtime/channel.rs` — AsyncReceiver / NotifySender
   - `src/core/net/ws.rs` — WebSocket 帧协议
   - `src/core/jsonrpc.rs` — JSON-RPC 2.0

2. **系统异步化**
   - `src/llm.rs` — stream_agent() 改为 async
   - `src/tools/mod.rs` — Tool::execute() 改为 async
   - 所有内置工具适配 async execute
   - `src/agent/agent.rs` — Agent::run() 改为 async
   - `src/main.rs` — Runtime 线程启动 Agent Future

3. **MCP 协议栈**
   - `src/mcp/types.rs` — 数据类型
   - `src/mcp/config.rs` — 配置加载
   - `src/mcp/transport/mod.rs` — Transport trait
   - `src/mcp/transport/stdio.rs` — stdio 传输
   - `src/mcp/client.rs` — McpClient 协议逻辑
   - `src/mcp/mod.rs` — McpManager

4. **Agent 集成**
   - McpToolProxy 注册
   - ListMcpResources / ReadMcpResource 工具
   - ListMcpPrompts / GetMcpPrompt 工具

5. **剩余传输**
   - `src/mcp/transport/sse.rs`
   - `src/mcp/transport/http.rs`
   - `src/mcp/transport/ws.rs`

6. **测试**
   - 单元测试：JSON-RPC 解析、WebSocket 帧编解码
   - 集成测试：stdio MCP 服务器端到端（`--features full_test`）
