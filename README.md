# viv

一个自我进化的 AI 编程 Agent。核心理念是"越用越好用"——Agent 在使用过程中持续积累经验、进化能力，随时间变得越来越顺手。

## 核心特性

- **自我进化** — 每次会话结束后自动提炼经验、更新记忆，下次更聪明
- **分层记忆** — Working / Session / Episodic / Semantic 四层记忆，动态注入相关上下文
- **Agent 循环** — tool_use → tool_result → 再次请求，支持 60+ 内置工具
- **MCP 集成** — 通过 stdio / SSE / HTTP / WebSocket 接入外部工具生态
- **Skill 系统** — 可扩展的技能库，按需加载注入上下文
- **零外部依赖** — JSON、TLS、HTTP、SSE、async runtime 全部从零实现，无任何第三方 crate
- **跨平台** — 支持 Linux、macOS、Windows

## 构建

需要系统已安装 OpenSSL（用于 TLS FFI 绑定）。

```bash
cargo build
```

## 运行

```bash
VIV_API_KEY=你的密钥 cargo run
```

### 环境变量

| 变量 | 必填 | 说明 |
|------|------|------|
| `VIV_API_KEY` | 是 | LLM API 密钥 |
| `VIV_BASE_URL` | 否 | API base URL（默认 api.anthropic.com） |
| `VIV_MODEL` | 否 | 所有模型档位的回退值 |
| `VIV_MODEL_FAST` | 否 | 快速模型（默认 claude-haiku-4-5） |
| `VIV_MODEL_MEDIUM` | 否 | 中等模型（默认 claude-sonnet-4-6） |
| `VIV_MODEL_SLOW` | 否 | 慢速模型（默认 claude-opus-4-6） |

## 测试

```bash
cargo test                         # 运行所有测试
cargo test --features full_test    # 包含 e2e 测试（调用真实 API）
```

## 架构

```
src/
├── main.rs              # 入口
├── lib.rs               # 模块导出 + Error/Result
├── error.rs             # 统一错误类型
├── json.rs              # JSON 解析器/序列化器
├── llm.rs               # LLM 客户端（fast/medium/slow 三档）
├── repl.rs              # REPL 主循环 + 行编辑器
├── event.rs             # epoll 事件循环
│
├── runtime/             # 自制 async executor + I/O reactor（零依赖）
│   ├── executor.rs      # 任务调度 + block_on + spawn
│   ├── reactor.rs       # epoll fd 注册 + Waker 映射
│   ├── task.rs          # Task + RawWaker vtable + JoinHandle
│   └── timer.rs         # timerfd sleep Future
│
├── agent/               # Agent 循环 + 进化引擎
│   ├── loop.rs          # tool_use → tool_result 主循环
│   ├── message.rs       # Message / ContentBlock 类型
│   ├── context.rs       # AgentContext
│   ├── prompt.rs        # System prompt 拼接逻辑
│   └── evolution.rs     # 自我进化：经验提炼 + 记忆更新
│
├── memory/              # 分层记忆系统
│   ├── store.rs         # .viv/memory/ 文件读写
│   ├── index.rs         # 记忆索引管理
│   ├── retrieval.rs     # 两阶段检索（关键词 + LLM 排序）
│   └── compaction.rs    # 上下文压缩
│
├── tools/               # Tool trait + 60+ 内置工具
│   ├── mod.rs           # Tool trait + 注册表
│   ├── bash.rs          # Bash
│   ├── file/            # Read / Write / Edit / Glob / Grep
│   └── ...
│
├── permissions/         # 权限模型
│   ├── rules.rs         # allow/deny/ask 规则匹配
│   └── classifier.rs    # AI 动态分类器
│
├── mcp/                 # MCP 协议栈
│   ├── client.rs        # MCP 客户端
│   └── transport/       # stdio / sse / http / ws
│
├── skills/              # Skill 加载 + 发现
│
├── terminal/            # TUI 组件
│   ├── raw_mode.rs
│   ├── input.rs
│   ├── output.rs
│   └── screen.rs
│
└── net/                 # 网络层
    ├── tcp.rs
    ├── async_tcp.rs     # 异步 TCP（集成 reactor）
    ├── tls.rs
    ├── http.rs
    └── sse.rs
```

### 记忆层级

```
L1 Working Memory    当前对话消息（内存，受 context 窗口限制）
L2 Session Memory    完整会话历史（.viv/sessions/）
L3 Episodic Memory   过去会话摘要（.viv/memory/episodes/）
L4 Semantic Memory   项目事实/用户偏好/学习规律（.viv/memory/knowledge/）
L5 Skill Memory      技能库（.viv/skills/）
```

## 配置

```
.viv/
├── settings.json    # MCP 服务器、权限规则配置
├── sessions/        # 会话历史
├── memory/          # 记忆库
│   ├── index.json
│   ├── episodes/
│   └── knowledge/
└── skills/          # 项目级 Skill
```

## 许可证

[Apache License 2.0](LICENSE)

## 参考项目

- https://github.com/openai/codex
- https://github.com/anthropics/claude-code
- https://github.com/crossterm-rs/crossterm
- https://github.com/ratatui/ratatui
- https://github.com/rustls/rustls
