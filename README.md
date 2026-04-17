# viv

A self-evolving AI programming agent. The core idea is "gets smarter the more you use it" — the agent continuously accumulates experience and evolves its capabilities over time.

## Features

- **Self-evolution** — automatically distills experience and updates memory after each session, becoming smarter over time
- **Layered memory** — four memory layers (Working / Session / Episodic / Semantic) with dynamic context injection
- **Agent loop** — tool_use → tool_result → next request, supporting 60+ built-in tools
- **MCP integration** — connect external tool ecosystems via stdio / SSE / HTTP / WebSocket
- **Skill system** — extensible skill library with on-demand context injection
- **Zero external dependencies** — JSON, TLS, HTTP, SSE, async runtime all built from scratch; no third-party crates
- **Cross-platform** — Linux, macOS, Windows

## Build

Requires OpenSSL installed on the system (for TLS FFI bindings).

```bash
cargo build
```

## Run

```bash
VIV_API_KEY=your_key cargo run
```

### Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `VIV_API_KEY` | Yes | LLM API key |
| `VIV_BASE_URL` | No | API base URL (default: api.anthropic.com) |
| `VIV_MODEL` | No | Fallback model for all tiers |
| `VIV_MODEL_FAST` | No | Fast model (default: claude-haiku-4-5) |
| `VIV_MODEL_MEDIUM` | No | Medium model (default: claude-sonnet-4-6) |
| `VIV_MODEL_SLOW` | No | Slow model (default: claude-opus-4-6) |

## Testing

```bash
cargo test                         # run all tests
cargo test --features full_test    # include e2e tests (calls real API)
```

## Architecture

```
src/
├── main.rs              # entry point
├── lib.rs               # module exports + Error/Result
├── error.rs             # unified error type
├── json.rs              # JSON parser/serializer
├── llm.rs               # LLM client (fast/medium/slow tiers)
├── repl.rs              # REPL main loop + line editor
├── event.rs             # epoll event loop
│
├── runtime/             # custom async executor + I/O reactor (zero deps)
│   ├── executor.rs      # task scheduling + block_on + spawn
│   ├── reactor.rs       # epoll fd registration + Waker mapping
│   ├── task.rs          # Task + RawWaker vtable + JoinHandle
│   └── timer.rs         # timerfd sleep Future
│
├── agent/               # agent loop + evolution engine
│   ├── run.rs           # tool_use → tool_result main loop
│   ├── message.rs       # Message / ContentBlock types
│   ├── context.rs       # AgentContext
│   ├── prompt.rs        # system prompt assembly
│   └── evolution.rs     # self-evolution: experience extraction + memory update
│
├── memory/              # layered memory system
│   ├── store.rs         # .viv/memory/ file I/O
│   ├── index.rs         # memory index management
│   ├── retrieval.rs     # two-stage retrieval (keyword + LLM ranking)
│   └── compaction.rs    # context compaction
│
├── tools/               # Tool trait + 60+ built-in tools
│   ├── mod.rs           # Tool trait + registry
│   ├── bash.rs          # Bash
│   ├── file/            # Read / Write / Edit / Glob / Grep
│   └── ...
│
├── permissions/         # permission model
│   ├── rules.rs         # allow/deny/ask rule matching
│   └── classifier.rs    # AI dynamic classifier
│
├── mcp/                 # MCP protocol stack
│   ├── client.rs        # MCP client
│   └── transport/       # stdio / sse / http / ws
│
├── skills/              # skill loading + discovery
│
├── terminal/            # TUI components
│   ├── raw_mode.rs
│   ├── input.rs
│   ├── output.rs
│   └── screen.rs
│
└── net/                 # network layer
    ├── tcp.rs
    ├── async_tcp.rs     # async TCP (integrated with reactor)
    ├── tls.rs
    ├── http.rs
    └── sse.rs
```

### Memory Layers

```
L1 Working Memory    current conversation messages (in-memory, context window limited)
L2 Session Memory    full session history (.viv/sessions/)
L3 Episodic Memory   past session summaries (.viv/memory/episodes/)
L4 Semantic Memory   project facts / user preferences / learned patterns (.viv/memory/knowledge/)
L5 Skill Memory      skill library (.viv/skills/)
```

## Configuration

```
.viv/
├── settings.json    # MCP servers, permission rules
├── sessions/        # session history
├── memory/          # memory store
│   ├── index.json
│   ├── episodes/
│   └── knowledge/
└── skills/          # project-level skills
```

## License

[Apache License 2.0](LICENSE)

## References

- https://github.com/openai/codex
- https://github.com/anthropics/claude-code
- https://github.com/crossterm-rs/crossterm
- https://github.com/ratatui/ratatui
- https://github.com/rustls/rustls
