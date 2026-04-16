# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**viv** — 一个自我进化的 AI 编程 Agent。核心理念是"越用越好用"，Agent 能够在使用过程中不断学习和进化。长期目标：部署到裸机的 AgentOS。

## Build Commands

```bash
cargo build                    # Debug build
cargo build --release          # Release build
cargo run                      # Build and run (需要 VIV_API_KEY)
cargo test                     # Run all tests
cargo test --features full_test     # 包含 e2e 测试（会调用真实 API，花钱）
cargo test --test llm_test     # 运行单个测试文件
cargo fmt                      # Format code
cargo clippy                   # Lint
```

## Environment Variables

```bash
VIV_API_KEY=xxx          # Required: LLM API key
VIV_BASE_URL=xxx         # Optional: API base URL (default: api.anthropic.com)
VIV_MODEL=xxx            # Optional: fallback model for all tiers
VIV_MODEL_FAST=xxx       # Optional: fast tier model (default: claude-haiku-4-5)
VIV_MODEL_MEDIUM=xxx     # Optional: medium tier model (default: claude-sonnet-4-6)
VIV_MODEL_SLOW=xxx       # Optional: slow tier model (default: claude-opus-4-6)
```

Model resolution: `VIV_MODEL_FAST` > `VIV_MODEL` > default value（其他两档同理）

## Architecture

零外部依赖，单 crate（edition 2024）。TLS 通过 FFI 调用系统 OpenSSL。

```
src/
├── main.rs              # 入口 → repl::run()
├── lib.rs               # 模块导出 + Error/Result 类型别名
├── error.rs             # 统一 Error 枚举
├── json.rs              # JSON 解析器/序列化器
├── llm.rs               # LLM 客户端（LlmConfig, LlmClient, ModelTier）
├── repl.rs              # REPL 主循环 + 行编辑器
├── event.rs             # epoll 事件循环封装
├── terminal/
│   ├── raw_mode.rs      # termios FFI + RAII 恢复
│   ├── input.rs         # 字节流 → KeyEvent 解析
│   ├── output.rs        # ANSI 转义序列输出
│   └── screen.rs        # 双缓冲 + diff 渲染
└── net/
    ├── tcp.rs           # TCP 连接
    ├── tls.rs           # OpenSSL FFI TLS 客户端
    ├── http.rs          # HTTP/1.1 请求/响应
    └── sse.rs           # Server-Sent Events 流解析
```

## Key Design Decisions

- **零依赖**：为未来裸机部署 (AgentOS) 做准备
- **Error 枚举**：不用 String，统一 `error::Error` 类型
- **测试目录镜像源码**：`tests/terminal/` 对应 `src/terminal/`
- **VIV_ 环境变量**：不绑定 Anthropic 命名
- **三档模型**：fast/medium/slow，Agent 按任务复杂度选择
- **条件编译 e2e 测试**：`--features full_test` 开启（调 API 花钱）

