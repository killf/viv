# viv

一个自我进化的 AI 编程 Agent。核心理念是"越用越好用"——Agent 在使用过程中不断学习和积累，随着时间推移变得越来越顺手。

长期目标是将 viv 作为 AgentOS 部署在裸机上，成为开发者的第二大脑。

## 特性

- **零外部依赖** — JSON 解析、终端控制、TLS、HTTP 客户端、SSE 解析全部从零实现，无任何第三方 crate
- **自我进化** — Agent 能够在使用过程中持续学习，越用越好用
- **跨平台** — 目标支持 Linux、macOS、Windows
- **极简内核** — 用 Rust nightly 编写，面向裸机部署设计

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
cargo test                       # 运行所有单元测试
cargo test --features full_test       # 包含 e2e 测试（调用真实 API）
```

## 架构概览

```
src/
├── main.rs              # 入口 → repl::run()
├── lib.rs               # 模块导出 + Error/Result
├── error.rs             # 统一错误类型
├── json.rs              # JSON 解析器/序列化器
├── llm.rs               # LLM 客户端（三档模型：fast/medium/slow）
├── repl.rs              # REPL 主循环 + 行编辑器
├── event.rs             # epoll 事件循环
├── terminal/
│   ├── raw_mode.rs      # termios 原始模式 + RAII 恢复
│   ├── input.rs         # 键盘输入解析
│   ├── output.rs        # ANSI 输出渲染
│   └── screen.rs        # 双缓冲屏幕管理
└── net/
    ├── tcp.rs           # TCP 连接
    ├── tls.rs           # TLS（OpenSSL FFI）
    ├── http.rs          # HTTP/1.1 客户端
    └── sse.rs           # Server-Sent Events 解析
```

## 许可证

[Apache License 2.0](LICENSE)
