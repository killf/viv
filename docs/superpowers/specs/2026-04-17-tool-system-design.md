# Tool System Design

**Goal:** Implement a real Tool execution layer for viv's agent loop — replacing the current stub with 12 built-in tools, tiered permissions, and Anthropic JSON schema integration.

**Architecture:** Static registry of `Box<dyn Tool>` trait objects, permission gating via `PermissionManager`, injected into `AgentContext`. The LLM receives a `tools` array in every agent request; tool results feed back as `ContentBlock::ToolResult` user messages per the existing agent loop design.

**Tech Stack:** Rust std only (zero external deps). Shell via `std::process::Command`. File I/O via `std::fs`. HTTP for WebFetch reuses existing `net/` stack.

---

## File Structure

```
src/tools/
├── mod.rs          # Tool trait + PermissionLevel + ToolRegistry
├── bash.rs         # Bash + BashBackground
├── todo.rs         # TodoWrite + TodoRead
├── web.rs          # WebFetch
└── file/
    ├── mod.rs
    ├── read.rs
    ├── write.rs
    ├── edit.rs     # Edit + MultiEdit
    ├── glob.rs
    ├── grep.rs
    └── ls.rs

src/permissions/
├── mod.rs
└── manager.rs      # PermissionManager
```

---

## Tool Trait

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> JsonValue;         // Anthropic JSON schema object
    fn execute(&self, input: &JsonValue) -> crate::Result<String>;
    fn permission_level(&self) -> PermissionLevel;
}

pub enum PermissionLevel {
    ReadOnly,   // auto-execute
    Write,      // ask once, remember for session
    Execute,    // ask once, remember for session
}
```

---

## ToolRegistry

```rust
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self
    pub fn register(&mut self, tool: Box<dyn Tool>)
    pub fn get(&self, name: &str) -> Option<&dyn Tool>
    pub fn to_api_json(&self) -> String   // JSON array for LLM "tools" field
    pub fn default_tools() -> Self        // registers all 12 built-in tools
}
```

`to_api_json` emits the Anthropic tools array format:
```json
[
  {
    "name": "bash",
    "description": "...",
    "input_schema": { "type": "object", "properties": { ... }, "required": [...] }
  }
]
```

---

## Permission Manager

```rust
pub struct PermissionManager {
    session_allowed: HashSet<String>,  // tool names allowed for this session
}

impl PermissionManager {
    pub fn check(
        &mut self,
        tool: &dyn Tool,
        input: &JsonValue,
        ask_fn: &mut dyn FnMut(&str, &JsonValue) -> bool,
    ) -> bool
}
```

- `ReadOnly` tools: always allowed, `ask_fn` never called.
- `Write` / `Execute` tools: if tool name already in `session_allowed`, allow immediately. Otherwise call `ask_fn(tool_name, input)`. If granted, insert into `session_allowed`.

---

## 12 Built-in Tools

| Tool | Permission | Key Input Fields | Notes |
|------|-----------|-----------------|-------|
| `bash` | Execute | `command: str`, `timeout_ms?: u64` | Captures stdout+stderr; default timeout 30 000 ms |
| `bash_background` | Execute | `command: str`, `description: str` | Spawns detached process; returns pid |
| `read` | ReadOnly | `file_path: str`, `offset?: u64`, `limit?: u64` | Returns content with line numbers |
| `write` | Write | `file_path: str`, `content: str` | Overwrites file; creates parent dirs |
| `edit` | Write | `file_path: str`, `old_string: str`, `new_string: str`, `replace_all?: bool` | Fails if `old_string` not found or not unique (unless `replace_all`) |
| `multi_edit` | Write | `file_path: str`, `edits: [{old_string, new_string, replace_all?}]` | Applies edits in order atomically |
| `glob` | ReadOnly | `pattern: str`, `path?: str` | Returns newline-separated matching paths |
| `grep` | ReadOnly | `pattern: str`, `path?: str`, `glob?: str`, `output_mode?: str` | Modes: `files_with_matches` (default), `content`, `count` |
| `ls` | ReadOnly | `path?: str` | Lists directory entries with type indicators |
| `todo_write` | Write | `todos: [{id, content, status}]` | Overwrites `.viv/todo.json` |
| `todo_read` | ReadOnly | — | Reads `.viv/todo.json`; returns `[]` if missing |
| `web_fetch` | Execute | `url: str`, `prompt: str` | Fetches URL via existing HTTP stack, extracts text, summarises with Fast LLM |

---

## AgentContext Changes

Add two fields:

```rust
pub struct AgentContext {
    // existing fields unchanged
    pub tool_registry: ToolRegistry,
    pub permission_manager: PermissionManager,
}
```

`AgentContext::new` initialises both:
```rust
tool_registry: ToolRegistry::default_tools(),
permission_manager: PermissionManager::default(),
```

---

## run_agent Changes

Signature:
```rust
pub fn run_agent(
    input: String,
    ctx: &mut AgentContext,
    ask_fn: &mut dyn FnMut(&str, &JsonValue) -> bool,
    mut on_text: impl FnMut(&str),
) -> Result<AgentOutput>
```

- Remove `tool_descriptions: &str` and `skill_contents: &str` parameters.
- `tool_descriptions` sourced from `ctx.tool_registry.to_api_json()`.
- Tool stub replaced with real dispatch:

```rust
let tool_results = stream_result.tool_uses.iter().map(|tu| {
    if let ContentBlock::ToolUse { id, name, input } = tu {
        let result = match ctx.tool_registry.get(name) {
            None => Err(crate::Error::Tool(format!("unknown tool: {}", name))),
            Some(tool) => {
                if ctx.permission_manager.check(tool, input, ask_fn) {
                    tool.execute(input)
                } else {
                    Err(crate::Error::Tool("permission denied".into()))
                }
            }
        };
        let (content, is_error) = match result {
            Ok(text) => (text, false),
            Err(e) => (e.to_string(), true),
        };
        ContentBlock::ToolResult {
            tool_use_id: id.clone(),
            content: vec![ContentBlock::Text(content)],
            is_error,
        }
    } else { unreachable!() }
}).collect();
```

---

## LLM Request Changes

`build_agent_request` in `llm.rs` gains a `tools_json: &str` parameter and adds the `tools` field to the request body:

```rust
fn build_agent_request(
    system_blocks: &[SystemBlock],
    messages: &[Message],
    tools_json: &str,        // ← new
    tier: ModelTier,
) -> HttpRequest
```

Body becomes:
```json
{
  "model": "...",
  "max_tokens": ...,
  "stream": true,
  "tools": [...],
  "system": [...],
  "messages": [...]
}
```

---

## Error Type Change

Add `Tool` variant to `error::Error`:

```rust
Error::Tool(String)   // tool execution errors (unknown tool, permission denied, exec failure)
```

---

## REPL Integration

`run_agent` call in `repl.rs` injects `ask_fn`:

```rust
let mut ask_fn = |tool_name: &str, input: &JsonValue| -> bool {
    // render permission prompt line at bottom of TUI
    // block-read a single keypress: 'y' = true, anything else = false
    render_permission_prompt(&mut backend, tool_name, input);
    read_yn_key(&mut backend)
};
run_agent(line, &mut agent_ctx, &mut ask_fn, |text| { ... });
```

`render_permission_prompt` writes a dim line: `Allow bash("ls -la")? [y/n]`

---

## Testing Strategy

- Unit tests per tool in `tests/tools/` mirroring `src/tools/`
- `bash`: test stdout capture, stderr capture, timeout, exit code non-zero → `is_error: true`
- `read`: test offset/limit, missing file → error
- `edit`: test replacement, old_string not found → error, replace_all
- `glob` / `grep`: test against temp dirs created in test setup
- `permission_manager`: test ReadOnly always passes, Write/Execute ask-then-remember flow
- `tool_registry`: test `to_api_json` produces valid JSON with correct schema shape
