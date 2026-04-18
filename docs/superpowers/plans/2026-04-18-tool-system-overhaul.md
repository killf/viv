# Tool System Overhaul Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Overhaul viv's tool system to align with Claude Code — rename tools, enrich descriptions, fix parameter gaps, add agent communication abstraction, and implement NotebookEdit/WebSearch/SubAgent tools.

**Architecture:** Top-down approach. First add the agent-to-agent channel abstraction (`bus/channel.rs`), then fix the framework layer (`ToolRegistry.to_api_json`, `default_tools_without`), then rename and enrich each existing tool, then add new tools (NotebookEdit, WebSearch, SubAgent) with the SubAgent using the new channel abstraction and `Agent::new_sub()`.

**Tech Stack:** Rust (edition 2024), zero external dependencies, custom async runtime, custom JSON parser, OpenSSL FFI for TLS.

**Spec:** `docs/superpowers/specs/2026-04-18-tool-system-overhaul-design.md`

---

## File Structure

### New Files
| File | Responsibility |
|------|---------------|
| `src/bus/channel.rs` | AgentHandle, AgentEndpoint, agent_channel() — agent-to-agent communication |
| `src/tools/notebook.rs` | NotebookEditTool — Jupyter notebook cell editing |
| `src/tools/search.rs` | WebSearchTool — Tavily API web search |
| `src/tools/agent.rs` | SubAgentTool — spawns ephemeral child agents via agent_channel |
| `tests/bus/channel_test.rs` | Tests for agent_channel bidirectional communication |
| `tests/tools/notebook_test.rs` | Tests for NotebookEditTool |
| `tests/tools/search_test.rs` | Tests for WebSearchTool (mocked HTTP) |
| `tests/tools/agent_test.rs` | Tests for SubAgentTool |

### Modified Files
| File | Changes |
|------|---------|
| `src/bus/mod.rs` | Add `pub mod channel;` |
| `src/tools/mod.rs` | Rewrite `to_api_json()`, add `default_tools_without()`, register new tools |
| `src/tools/file/read.rs` | Rename to `Read`, enrich description, add binary detection + PDF pages |
| `src/tools/file/write.rs` | Rename to `Write`, enrich description, return line count |
| `src/tools/file/edit.rs` | Rename to `Edit`, enrich description for both EditTool and MultiEditTool |
| `src/tools/file/glob.rs` | Fix sorting (mtime), add default ignores, fix description |
| `src/tools/file/grep.rs` | Add `context` alias, type expansion map, multiline portability |
| `src/tools/file/ls.rs` | Enrich description |
| `src/tools/bash.rs` | Enrich description, remove `dangerouslyDisableSandbox` |
| `src/tools/todo.rs` | Remove `id`/`priority`, add `activeForm` |
| `src/tools/web.rs` | HTML→Markdown converter, increase truncation to 16000 |
| `src/agent/agent.rs` | Add `new_sub()`, update LSP name matching, add Agent tool concurrency |
| `src/core/runtime/mod.rs` | Add `join_all`, `join` combinators |
| `tests/tools/registry_test.rs` | Update `to_api_json` assertion format |
| `tests/tools/file/read_test.rs` | Update tool name in imports if needed |
| `tests/tools/file/write_test.rs` | Update tool name in imports if needed |
| `tests/tools/file/edit_test.rs` | Update tool name in imports if needed |
| `tests/tools/todo_test.rs` | Update schema (remove id/priority, add activeForm) |
| `tests/bus/mod.rs` | Add `mod channel_test;` |
| `tests/tools/mod.rs` | Add `mod notebook_test; mod search_test; mod agent_test;` |

---

## Task 1: Agent Channel Abstraction

**Files:**
- Create: `src/bus/channel.rs`
- Modify: `src/bus/mod.rs:1`
- Create: `tests/bus/channel_test.rs`
- Modify: `tests/bus/mod.rs`

- [ ] **Step 1: Write failing tests for agent_channel**

```rust
// tests/bus/channel_test.rs
use viv::bus::channel::{agent_channel, AgentHandle, AgentEndpoint};
use viv::bus::{AgentEvent, AgentMessage};
use viv::core::runtime::block_on_local;

#[test]
fn agent_channel_send_event_receive_in_endpoint() {
    let (handle, endpoint) = agent_channel();
    handle.tx.send(AgentEvent::Input("hello".into())).unwrap();
    // Use block_on_local to async-receive
    let event = block_on_local(Box::pin(endpoint.rx.recv())).unwrap();
    match event {
        AgentEvent::Input(s) => assert_eq!(s, "hello"),
        other => panic!("expected Input, got {:?}", other),
    }
}

#[test]
fn agent_channel_send_message_receive_in_handle() {
    let (handle, endpoint) = agent_channel();
    endpoint.tx.send(AgentMessage::TextChunk("chunk".into())).unwrap();
    match handle.rx.try_recv() {
        Ok(AgentMessage::TextChunk(s)) => assert_eq!(s, "chunk"),
        other => panic!("expected TextChunk, got {:?}", other),
    }
}

#[test]
fn agent_channel_bidirectional_permission_flow() {
    let (handle, endpoint) = agent_channel();
    // Child requests permission
    endpoint.tx.send(AgentMessage::PermissionRequest {
        tool: "Bash".into(),
        input: "command=ls".into(),
    }).unwrap();
    // Parent receives
    match handle.rx.try_recv() {
        Ok(AgentMessage::PermissionRequest { tool, .. }) => assert_eq!(tool, "Bash"),
        other => panic!("expected PermissionRequest, got {:?}", other),
    }
    // Parent responds
    handle.tx.send(AgentEvent::PermissionResponse(true)).unwrap();
    // Child receives response via async recv
    let event = block_on_local(Box::pin(endpoint.rx.recv())).unwrap();
    match event {
        AgentEvent::PermissionResponse(true) => {},
        other => panic!("expected PermissionResponse(true), got {:?}", other),
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test bus_tests channel_test -- --nocapture`
Expected: compilation error — `channel` module not found.

- [ ] **Step 3: Implement agent_channel**

```rust
// src/bus/channel.rs
use crate::bus::{AgentEvent, AgentMessage};
use crate::core::runtime::channel::{async_channel, AsyncReceiver, NotifySender};
use std::sync::mpsc;

/// Parent/holder side — sends events to child agent, receives messages from it.
pub struct AgentHandle {
    pub tx: NotifySender<AgentEvent>,
    pub rx: mpsc::Receiver<AgentMessage>,
}

/// Child agent side — receives events from parent, sends messages to parent.
pub struct AgentEndpoint {
    pub rx: AsyncReceiver<AgentEvent>,
    pub tx: mpsc::Sender<AgentMessage>,
}

/// Create a bidirectional channel for agent-to-agent communication.
/// Uses NotifySender/AsyncReceiver for the event direction (parent→child)
/// so the child agent can async-await on incoming events.
pub fn agent_channel() -> (AgentHandle, AgentEndpoint) {
    let (event_tx, event_rx) = async_channel();
    let (msg_tx, msg_rx) = mpsc::channel();
    (
        AgentHandle { tx: event_tx, rx: msg_rx },
        AgentEndpoint { rx: event_rx, tx: msg_tx },
    )
}
```

- [ ] **Step 4: Add module declarations**

In `src/bus/mod.rs`, add at line 1:
```rust
pub mod channel;
```

In `tests/bus/mod.rs`, add:
```rust
mod channel_test;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test bus_tests channel_test -- --nocapture`
Expected: all 3 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/bus/channel.rs src/bus/mod.rs tests/bus/channel_test.rs tests/bus/mod.rs
git commit -m "feat: add agent_channel for agent-to-agent communication"
```

---

## Task 2: ToolRegistry Improvements

**Files:**
- Modify: `src/tools/mod.rs:49-63` (to_api_json), `src/tools/mod.rs:65-90` (default_tools)
- Modify: `tests/tools/registry_test.rs:48-55`

- [ ] **Step 1: Write failing test for to_api_json with special chars**

Add to `tests/tools/registry_test.rs`:

```rust
struct QuoteTool;
impl Tool for QuoteTool {
    fn name(&self) -> &str { "quote" }
    fn description(&self) -> &str { "Handles \"quotes\" and \\backslashes" }
    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{"type":"object","properties":{}}"#).unwrap()
    }
    fn execute(&self, _input: &JsonValue)
        -> Pin<Box<dyn Future<Output = viv::Result<String>> + Send + '_>> {
        Box::pin(async { Ok(String::new()) })
    }
    fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }
}

#[test]
fn to_api_json_escapes_special_characters() {
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(QuoteTool));
    let json = reg.to_api_json();
    // Must be valid JSON — parse it back
    let parsed = JsonValue::parse(&json).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    let tool = &arr[0];
    assert_eq!(tool.get("name").unwrap().as_str().unwrap(), "quote");
    assert!(tool.get("description").unwrap().as_str().unwrap().contains("\"quotes\""));
}

#[test]
fn default_tools_without_excludes_named_tool() {
    let llm_config = viv::llm::LLMConfig::from_env();
    // Skip if no API key (CI environment)
    if llm_config.is_err() { return; }
    let llm = std::sync::Arc::new(viv::llm::LLMClient::new(llm_config.unwrap()));
    let reg = ToolRegistry::default_tools_without("Bash", llm);
    assert!(reg.get("Bash").is_none());
    assert!(reg.get("Read").is_some()); // Other tools still present
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test tools_tests registry_test -- --nocapture`
Expected: `default_tools_without` not found; `to_api_json_escapes` may fail on parse.

- [ ] **Step 3: Rewrite to_api_json using JsonValue**

Replace `src/tools/mod.rs` lines 49-63:

```rust
    pub fn to_api_json(&self) -> String {
        let tools: Vec<JsonValue> = self
            .tools
            .iter()
            .map(|t| {
                JsonValue::Object(vec![
                    ("name".into(), JsonValue::Str(t.name().into())),
                    ("description".into(), JsonValue::Str(t.description().into())),
                    ("input_schema".into(), t.input_schema()),
                ])
            })
            .collect();
        format!("{}", JsonValue::Array(tools))
    }
```

- [ ] **Step 4: Add default_tools_without**

Add after `default_tools()` in `src/tools/mod.rs`:

```rust
    pub fn default_tools_without(exclude: &str, llm: std::sync::Arc<crate::llm::LLMClient>) -> Self {
        let mut reg = Self::default_tools(llm);
        reg.tools.retain(|t| t.name() != exclude);
        reg
    }
```

- [ ] **Step 5: Update existing test assertion**

In `tests/tools/registry_test.rs`, update `registry_to_api_json_has_required_fields`:

```rust
#[test]
fn registry_to_api_json_has_required_fields() {
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(EchoTool));
    let json = reg.to_api_json();
    // Now it's proper JSON — parse it
    let parsed = JsonValue::parse(&json).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0].get("name").unwrap().as_str().unwrap(), "echo");
    assert!(arr[0].get("description").is_some());
    assert!(arr[0].get("input_schema").is_some());
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test --test tools_tests registry_test -- --nocapture`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/tools/mod.rs tests/tools/registry_test.rs
git commit -m "feat: rewrite to_api_json with JsonValue, add default_tools_without"
```

---

## Task 3: Tool Name Alignment (Read/Write/Edit)

**Files:**
- Modify: `src/tools/file/read.rs:11`
- Modify: `src/tools/file/write.rs:11`
- Modify: `src/tools/file/edit.rs:11` (EditTool) and `edit.rs:92` (MultiEditTool name reference if needed)
- Modify: `src/agent/agent.rs:322`
- Modify: `tests/tools/file/read_test.rs`, `write_test.rs`, `edit_test.rs` (if they match on tool name)

- [ ] **Step 1: Write test verifying new names**

Add to `tests/tools/registry_test.rs`:

```rust
#[test]
fn default_tools_have_claude_code_names() {
    let llm_config = viv::llm::LLMConfig::from_env();
    if llm_config.is_err() { return; }
    let llm = std::sync::Arc::new(viv::llm::LLMClient::new(llm_config.unwrap()));
    let reg = ToolRegistry::default_tools(llm);
    // New names
    assert!(reg.get("Read").is_some(), "Expected 'Read' tool");
    assert!(reg.get("Write").is_some(), "Expected 'Write' tool");
    assert!(reg.get("Edit").is_some(), "Expected 'Edit' tool");
    // Old names should NOT exist
    assert!(reg.get("FileRead").is_none(), "FileRead should be renamed");
    assert!(reg.get("FileWrite").is_none(), "FileWrite should be renamed");
    assert!(reg.get("FileEdit").is_none(), "FileEdit should be renamed");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test tools_tests default_tools_have_claude_code_names -- --nocapture`
Expected: FAIL — old names still present.

- [ ] **Step 3: Rename tools**

In `src/tools/file/read.rs:11`, change:
```rust
    fn name(&self) -> &str {
        "Read"
    }
```

In `src/tools/file/write.rs:11`, change:
```rust
    fn name(&self) -> &str {
        "Write"
    }
```

In `src/tools/file/edit.rs:11`, change:
```rust
    fn name(&self) -> &str {
        "Edit"
    }
```

- [ ] **Step 4: Update LSP notification in agent.rs**

In `src/agent/agent.rs:322`, change:
```rust
                    if matches!(name.as_str(), "Edit" | "Write" | "MultiEdit") {
```

- [ ] **Step 5: Run all tests**

Run: `cargo test`
Expected: all pass (existing tests use `ReadTool`/`WriteTool`/`EditTool` struct names, not string names).

- [ ] **Step 6: Commit**

```bash
git add src/tools/file/read.rs src/tools/file/write.rs src/tools/file/edit.rs src/agent/agent.rs tests/tools/registry_test.rs
git commit -m "feat: rename FileRead/FileWrite/FileEdit to Read/Write/Edit"
```

---

## Task 4: Enrich Descriptions — File Tools (Read, Write, Edit, MultiEdit)

**Files:**
- Modify: `src/tools/file/read.rs:14-16` (description)
- Modify: `src/tools/file/write.rs:14-16`
- Modify: `src/tools/file/edit.rs:14-16` and `edit.rs:95-97`

- [ ] **Step 1: Update Read description**

Replace `ReadTool::description()` in `src/tools/file/read.rs`:

```rust
    fn description(&self) -> &str {
        "Reads a file from the local filesystem.\n\n- The file_path parameter must be an absolute path, not a relative path\n- By default, it reads up to 2000 lines starting from the beginning of the file\n- You can optionally specify a line offset and limit (especially handy for long files), but it's recommended to read the whole file by not providing these parameters\n- Results are returned using cat -n format, with line numbers starting at 1\n- This tool can read PDF files (.pdf) when the pages parameter is provided\n- For binary files, a message is returned instead of raw content\n- If you read a file that exists but has empty contents you will receive a warning"
    }
```

- [ ] **Step 2: Update Write description**

Replace `WriteTool::description()` in `src/tools/file/write.rs`:

```rust
    fn description(&self) -> &str {
        "Writes a file to the local filesystem.\n\n- This tool will overwrite the existing file if there is one at the provided path\n- If this is an existing file, you MUST use the Read tool first to read the file's contents. This tool will fail if you did not read the file first\n- Prefer the Edit tool for modifying existing files — it only sends the diff. Only use this tool to create new files or for complete rewrites\n- NEVER create documentation files (*.md) or README files unless explicitly requested by the User"
    }
```

- [ ] **Step 3: Update Edit description**

Replace `EditTool::description()` in `src/tools/file/edit.rs`:

```rust
    fn description(&self) -> &str {
        "Performs exact string replacements in files.\n\nUsage:\n- You must use your Read tool at least once in the conversation before editing. This tool will error if you attempt an edit without reading the file.\n- When editing text from Read tool output, ensure you preserve the exact indentation (tabs/spaces) as it appears AFTER the line number prefix.\n- ALWAYS prefer editing existing files in the codebase. NEVER write new files unless explicitly required.\n- The edit will FAIL if `old_string` is not unique in the file. Either provide a larger string with more surrounding context to make it unique or use `replace_all` to change every instance of `old_string`.\n- Use `replace_all` for replacing and renaming strings across the file."
    }
```

- [ ] **Step 4: Update MultiEdit description**

Replace `MultiEditTool::description()` in `src/tools/file/edit.rs`:

```rust
    fn description(&self) -> &str {
        "Performs multiple exact string replacements in a single file atomically.\n\nAll edits are applied in sequence. If any edit fails (old_string not found or not unique), none of the edits are written — the file remains unchanged.\n\nEach edit must have a unique `old_string` in the current state of the file (after prior edits in the sequence). Prefer this over multiple Edit calls when changing the same file."
    }
```

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: all pass (descriptions are strings, no behavior change).

- [ ] **Step 6: Commit**

```bash
git add src/tools/file/read.rs src/tools/file/write.rs src/tools/file/edit.rs
git commit -m "feat: enrich descriptions for Read, Write, Edit, MultiEdit tools"
```

---

## Task 5: Enrich Descriptions — Bash, Glob, Grep, LS

**Files:**
- Modify: `src/tools/bash.rs:17-18` (description), `bash.rs:29` (remove dangerouslyDisableSandbox)
- Modify: `src/tools/file/glob.rs:15-17`
- Modify: `src/tools/file/grep.rs:15-17`
- Modify: `src/tools/file/ls.rs:14-16`

- [ ] **Step 1: Update Bash description and remove dangerouslyDisableSandbox**

Replace `BashTool::description()`:

```rust
    fn description(&self) -> &str {
        "Executes a given bash command and returns its output.\n\nIMPORTANT: Avoid using this tool to run `find`, `grep`, `cat`, `head`, `tail`, `sed`, `awk`, or `echo` commands — use the dedicated tools instead.\n\n- Quote file paths containing spaces with double quotes\n- Try to maintain current working directory using absolute paths and avoiding usage of `cd`\n- You may specify an optional timeout in milliseconds (up to 600000ms / 10 minutes). Default is 120000ms (2 minutes).\n- Use run_in_background for long-running processes\n- For git commands: prefer creating new commits rather than amending; never skip hooks (--no-verify)"
    }
```

Remove `dangerouslyDisableSandbox` from `input_schema()` — replace the schema:

```rust
    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "command":{"type":"string","description":"The command to execute"},
                "timeout":{"type":"number","description":"Optional timeout in milliseconds (max 600000). Default: 120000."},
                "description":{"type":"string","description":"Clear, concise description of what this command does in active voice."},
                "run_in_background":{"type":"boolean","description":"Set to true to run the command in the background. Returns the PID. Default: false."}
            },
            "required":["command"]
        }"#).unwrap()
    }
```

- [ ] **Step 2: Update Glob description**

Replace `GlobTool::description()`:

```rust
    fn description(&self) -> &str {
        "Fast file pattern matching tool that works with any codebase size.\n\n- Supports glob patterns like \"**/*.js\" or \"src/**/*.ts\"\n- Returns matching file paths sorted by modification time (most recent first)\n- Automatically ignores .git/, node_modules/, and target/ directories\n- Use this tool when you need to find files by name patterns"
    }
```

- [ ] **Step 3: Update Grep description**

Replace `GrepTool::description()`:

```rust
    fn description(&self) -> &str {
        "A powerful search tool built on grep.\n\n- Supports full regex syntax (e.g., \"log.*Error\", \"function\\s+\\w+\")\n- Filter files with glob parameter (e.g., \"*.js\", \"**/*.tsx\") or type parameter (e.g., \"js\", \"py\", \"rust\")\n- Output modes: \"content\" shows matching lines, \"files_with_matches\" shows only file paths (default), \"count\" shows match counts\n- Use -A/-B/-C or context for context lines around matches\n- Pattern syntax: Uses extended regex (ERE)"
    }
```

- [ ] **Step 4: Update LS description**

Replace `LsTool::description()`:

```rust
    fn description(&self) -> &str {
        "Lists files and directories in a given path.\n\n- Directories are shown with a trailing slash\n- Entries are sorted alphabetically\n- This tool can only list directories, not read files. To read a file, use the Read tool\n- Prefer Glob for pattern-based file discovery. Use LS when you want to see the full contents of a specific directory"
    }
```

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/tools/bash.rs src/tools/file/glob.rs src/tools/file/grep.rs src/tools/file/ls.rs
git commit -m "feat: enrich descriptions for Bash, Glob, Grep, LS tools"
```

---

## Task 6: Glob — Fix Sorting and Add Default Ignores

**Files:**
- Modify: `src/tools/file/glob.rs:42-51` (sort by mtime), `glob.rs:60-99` (add ignores)
- Modify: `tests/tools/file/glob_test.rs`

- [ ] **Step 1: Write failing test for mtime sorting**

Add to `tests/tools/file/glob_test.rs`:

```rust
#[test]
fn glob_results_sorted_by_modification_time() {
    let dir = tempdir();
    let a = dir.join("a.txt");
    let b = dir.join("b.txt");
    let c = dir.join("c.txt");
    // Create in order: a, b, c — then touch a to make it newest
    fs::write(&a, "a").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    fs::write(&b, "b").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    fs::write(&c, "c").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(50));
    // Touch a — rewrite to update mtime
    fs::write(&a, "a updated").unwrap();

    let input = JsonValue::parse(&format!(
        r#"{{"pattern":"*.txt","path":"{}"}}"#, json_path(&dir)
    )).unwrap();
    let result = poll_to_completion(GlobTool.execute(&input)).unwrap();
    let lines: Vec<&str> = result.lines().collect();
    // a.txt should be first (most recently modified)
    assert!(lines[0].ends_with("a.txt"), "Expected a.txt first (most recent), got: {}", lines[0]);
}

#[test]
fn glob_ignores_git_directory() {
    let dir = tempdir();
    fs::create_dir_all(dir.join(".git/objects")).unwrap();
    fs::write(dir.join(".git/objects/abc"), "git object").unwrap();
    fs::write(dir.join("real.txt"), "real").unwrap();

    let input = JsonValue::parse(&format!(
        r#"{{"pattern":"**/*","path":"{}"}}"#, json_path(&dir)
    )).unwrap();
    let result = poll_to_completion(GlobTool.execute(&input)).unwrap();
    assert!(!result.contains(".git"), "Should not include .git contents");
    assert!(result.contains("real.txt"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test tools_tests glob_test -- --nocapture`
Expected: FAIL — sorting is alphabetical, .git not ignored.

- [ ] **Step 3: Implement mtime sorting and default ignores**

In `src/tools/file/glob.rs`, replace the sort + result section (lines 42-51):

```rust
            let mut matches: Vec<PathBuf> = vec![];
            let parts: Vec<&str> = pattern.split('/').collect();
            walk_glob(Path::new(root), &parts, &mut matches)
                .map_err(|e| Error::Tool(format!("glob walk: {}", e)))?;
            // Sort by modification time, most recent first
            matches.sort_by(|a, b| {
                let ma = a.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                let mb = b.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                mb.cmp(&ma)
            });
            Ok(matches
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n"))
```

Add ignore logic at the top of `walk_glob` (after `let seg = parts[0];`):

```rust
const IGNORED_DIRS: &[&str] = &[".git", "node_modules", "target"];

fn walk_glob(dir: &Path, parts: &[&str], out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if parts.is_empty() {
        return Ok(());
    }
    let seg = parts[0];
    let rest = &parts[1..];

    if seg == "**" {
        // ... existing ** logic, but filter ignored dirs
```

In all `read_dir` loops inside `walk_glob` and `collect_all`, add:

```rust
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if p.is_dir() && IGNORED_DIRS.contains(&name) {
                continue;
            }
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test tools_tests glob_test -- --nocapture`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/file/glob.rs tests/tools/file/glob_test.rs
git commit -m "fix: glob sorts by mtime, ignores .git/node_modules/target"
```

---

## Task 7: Grep — Add context Alias and Type Expansion Map

**Files:**
- Modify: `src/tools/file/grep.rs:19-38` (schema), `grep.rs:47-71` (param parsing), `grep.rs:109-114` (type expansion)
- Modify: `tests/tools/file/grep_test.rs`

- [ ] **Step 1: Write failing tests**

Add to `tests/tools/file/grep_test.rs`:

```rust
#[test]
fn grep_context_alias_works_like_dash_c() {
    let dir = tempdir();
    let f = dir.join("f.txt");
    fs::write(&f, "aaa\nbbb\nccc\nddd\neee\n").unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"pattern":"ccc","path":"{}","output_mode":"content","context":1}}"#,
        json_path(&dir)
    )).unwrap();
    let result = poll_to_completion(GrepTool.execute(&input)).unwrap();
    assert!(result.contains("bbb"), "context=1 should show line before match");
    assert!(result.contains("ddd"), "context=1 should show line after match");
}

#[test]
fn grep_type_js_expands_to_multiple_extensions() {
    let dir = tempdir();
    fs::write(dir.join("a.js"), "target").unwrap();
    fs::write(dir.join("b.jsx"), "target").unwrap();
    fs::write(dir.join("c.mjs"), "target").unwrap();
    fs::write(dir.join("d.py"), "target").unwrap();

    let input = JsonValue::parse(&format!(
        r#"{{"pattern":"target","path":"{}","type":"js"}}"#,
        json_path(&dir)
    )).unwrap();
    let result = poll_to_completion(GrepTool.execute(&input)).unwrap();
    assert!(result.contains("a.js"));
    assert!(result.contains("b.jsx"));
    assert!(result.contains("c.mjs"));
    assert!(!result.contains("d.py"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test tools_tests grep_test -- --nocapture`
Expected: FAIL — `context` param not recognized; `type: "js"` only matches `*.js`.

- [ ] **Step 3: Add context alias in parameter parsing**

In `src/tools/file/grep.rs`, after the `context` variable is read (line 62), add:

```rust
            let context_alias = input.get("context").and_then(|v| v.as_i64()).unwrap_or(0);
            let context = input.get("-C").and_then(|v| v.as_i64()).unwrap_or(0).max(context_alias);
```

Also add `context` to the schema properties:

```json
"context":{"type":"number","description":"Alias for -C. Number of lines to show before and after each match."}
```

- [ ] **Step 4: Add type expansion map**

In `src/tools/file/grep.rs`, add a helper function:

```rust
fn expand_type_filter(t: &str) -> Vec<String> {
    match t {
        "js" => vec!["*.js", "*.jsx", "*.mjs", "*.cjs"],
        "ts" => vec!["*.ts", "*.tsx", "*.mts", "*.cts"],
        "py" => vec!["*.py", "*.pyi"],
        "rs" => vec!["*.rs"],
        "go" => vec!["*.go"],
        "java" => vec!["*.java"],
        "c" => vec!["*.c", "*.h"],
        "cpp" => vec!["*.cpp", "*.cc", "*.cxx", "*.hpp", "*.hh", "*.hxx"],
        "rb" => vec!["*.rb"],
        "sh" => vec!["*.sh", "*.bash", "*.zsh"],
        other => vec![&format!("*.{}", other)],
    }.into_iter().map(|s| s.to_string()).collect()
}
```

Replace the type filter section (lines 109-114):

```rust
            if let Some(t) = type_filter {
                for ext in expand_type_filter(t) {
                    cmd.arg(format!("--include={}", ext));
                }
            } else if let Some(g) = glob_filter {
                cmd.arg(format!("--include={}", g));
            }
```

- [ ] **Step 5: Run tests**

Run: `cargo test --test tools_tests grep_test -- --nocapture`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/tools/file/grep.rs tests/tools/file/grep_test.rs
git commit -m "feat: grep adds context alias and type expansion map"
```

---

## Task 8: Read — Binary Detection and PDF Pages

**Files:**
- Modify: `src/tools/file/read.rs:31-65`
- Modify: `tests/tools/file/read_test.rs`

- [ ] **Step 1: Write failing tests**

Add to `tests/tools/file/read_test.rs`:

```rust
#[test]
fn read_binary_file_returns_message_not_crash() {
    let dir = tempdir();
    let path = dir.join("binary.bin");
    // Write bytes with NUL characters
    fs::write(&path, b"\x89PNG\r\n\x1a\n\x00\x00binary").unwrap();
    let input = JsonValue::parse(&format!(r#"{{"file_path":"{}"}}"#, json_path(&path))).unwrap();
    let result = poll_to_completion(ReadTool.execute(&input)).unwrap();
    assert!(result.to_lowercase().contains("binary"), "Should indicate binary file: {}", result);
}

#[test]
fn read_empty_file_returns_warning() {
    let dir = tempdir();
    let path = dir.join("empty.txt");
    fs::write(&path, "").unwrap();
    let input = JsonValue::parse(&format!(r#"{{"file_path":"{}"}}"#, json_path(&path))).unwrap();
    let result = poll_to_completion(ReadTool.execute(&input)).unwrap();
    assert!(result.contains("empty") || result.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test tools_tests read_test -- --nocapture`
Expected: `read_binary_file` likely panics or returns garbled output.

- [ ] **Step 3: Implement binary detection and PDF support**

Replace `ReadTool::execute()` body in `src/tools/file/read.rs`:

```rust
        let input = input.clone();
        Box::pin(async move {
            let path = input
                .get("file_path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Tool("missing 'file_path'".into()))?;

            // PDF handling via pdftotext
            if path.ends_with(".pdf") {
                let pages = input.get("pages").and_then(|v| v.as_str());
                return read_pdf(path, pages);
            }

            // Binary detection: read first 512 bytes, check for NUL
            let raw = std::fs::read(path)
                .map_err(|e| Error::Tool(format!("cannot read '{}': {}", path, e)))?;
            if raw.iter().take(512).any(|&b| b == 0) {
                return Ok(format!("Binary file '{}' — cannot display as text ({} bytes)", path, raw.len()));
            }

            let content = String::from_utf8_lossy(&raw);
            if content.is_empty() {
                return Ok(format!("File '{}' exists but is empty", path));
            }

            let offset = input
                .get("offset")
                .and_then(|v| v.as_i64())
                .unwrap_or(1)
                .max(1) as usize;
            let limit = input
                .get("limit")
                .and_then(|v| v.as_i64())
                .map(|n| n as usize)
                .unwrap_or(2000);

            let lines: Vec<&str> = content.lines().collect();
            let start = (offset - 1).min(lines.len());
            let end = (start + limit).min(lines.len());

            let mut out = String::new();
            for (i, line) in lines[start..end].iter().enumerate() {
                out.push_str(&format!("{}\t{}\n", start + i + 1, line));
            }
            Ok(out)
        })
```

Add `read_pdf` helper:

```rust
fn read_pdf(path: &str, pages: Option<&str>) -> crate::Result<String> {
    use std::process::Command;
    let mut cmd = Command::new("pdftotext");
    if let Some(range) = pages {
        // Parse "1-5" or "3"
        let parts: Vec<&str> = range.split('-').collect();
        if let Some(first) = parts.first() {
            cmd.arg("-f").arg(first);
        }
        if let Some(last) = parts.get(1) {
            cmd.arg("-l").arg(last);
        }
    }
    cmd.arg(path).arg("-"); // output to stdout
    let output = cmd.output().map_err(|e| {
        Error::Tool(format!(
            "pdftotext not found or failed: {}. Install poppler-utils for PDF support.",
            e
        ))
    })?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Tool(format!("pdftotext error: {}", err.trim())));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test tools_tests read_test -- --nocapture`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/file/read.rs tests/tools/file/read_test.rs
git commit -m "feat: Read tool detects binary files, supports PDF via pdftotext"
```

---

## Task 9: Write — Return Line Count

**Files:**
- Modify: `src/tools/file/write.rs:49`

- [ ] **Step 1: Write failing test**

Add to `tests/tools/file/write_test.rs`:

```rust
#[test]
fn write_returns_line_count() {
    let dir = tempdir();
    let path = dir.join("out.txt");
    let input = JsonValue::parse(&format!(
        r#"{{"file_path":"{}","content":"line1\nline2\nline3\n"}}"#,
        json_path(&path)
    )).unwrap();
    let result = poll_to_completion(WriteTool.execute(&input)).unwrap();
    assert!(result.contains("3 lines") || result.contains("3 line"), "Should mention line count: {}", result);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test tools_tests write_test::write_returns_line_count -- --nocapture`
Expected: FAIL — currently returns bytes only.

- [ ] **Step 3: Update write result message**

In `src/tools/file/write.rs:49`, change:

```rust
            let line_count = content.lines().count();
            Ok(format!("Wrote {} bytes ({} lines) to {}", content.len(), line_count, path))
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test tools_tests write_test -- --nocapture`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/file/write.rs tests/tools/file/write_test.rs
git commit -m "feat: Write tool returns line count in result message"
```

---

## Task 10: TodoWrite — Align with Claude Code Format

**Files:**
- Modify: `src/tools/todo.rs:26-46` (schema)
- Modify: `tests/tools/todo_test.rs`

- [ ] **Step 1: Write failing test with new format**

Add to `tests/tools/todo_test.rs`:

```rust
#[test]
fn todo_write_accepts_claude_code_format() {
    let dir = tempdir();
    let path = dir.join("todo.json");
    let tool = TodoWriteTool::new(path.clone());
    let input = JsonValue::parse(r#"{
        "todos": [
            {"content": "Fix bug", "status": "in_progress", "activeForm": "Fixing bug"},
            {"content": "Write tests", "status": "pending", "activeForm": "Writing tests"}
        ]
    }"#).unwrap();
    let result = poll_to_completion(tool.execute(&input)).unwrap();
    assert!(result.contains("2 todo(s)"));

    // Verify written JSON
    let stored = std::fs::read_to_string(&path).unwrap();
    let parsed = JsonValue::parse(&stored).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0].get("content").unwrap().as_str().unwrap(), "Fix bug");
    assert_eq!(arr[0].get("activeForm").unwrap().as_str().unwrap(), "Fixing bug");
    // Should NOT have id or priority fields
    assert!(arr[0].get("id").is_none());
    assert!(arr[0].get("priority").is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test tools_tests todo_test::todo_write_accepts_claude_code_format -- --nocapture`
Expected: FAIL — schema requires `id`.

- [ ] **Step 3: Update TodoWrite schema**

In `src/tools/todo.rs`, replace the schema (lines 26-46):

```rust
    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "todos":{
                    "type":"array",
                    "description":"The updated todo list",
                    "items":{
                        "type":"object",
                        "properties":{
                            "content":{"type":"string","description":"Description of the task"},
                            "status":{"type":"string","description":"pending | in_progress | completed"},
                            "activeForm":{"type":"string","description":"Present continuous form shown during execution (e.g. 'Running tests')"}
                        },
                        "required":["content","status","activeForm"]
                    }
                }
            },
            "required":["todos"]
        }"#).unwrap()
    }
```

Update the description too:

```rust
    fn description(&self) -> &str {
        "Use this tool to create and manage a structured task list for the current coding session.\n\nTask descriptions must have two forms:\n- content: The imperative form describing what needs to be done (e.g., \"Run tests\")\n- activeForm: The present continuous form shown during execution (e.g., \"Running tests\")\n\nTask states: pending, in_progress, completed. Only one task should be in_progress at a time. Mark tasks complete immediately after finishing."
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test tools_tests todo_test -- --nocapture`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/todo.rs tests/tools/todo_test.rs
git commit -m "feat: TodoWrite aligns with Claude Code format (content/status/activeForm)"
```

---

## Task 11: WebFetch — HTML to Markdown Converter

**Files:**
- Modify: `src/tools/web.rs:58` (truncation), `web.rs:134-160` (replace strip_html)

- [ ] **Step 1: Write failing test for HTML→Markdown**

Create or add to a test file. Since WebFetch needs an LLM client, test the `html_to_markdown` function directly:

Add to `tests/tools/mod.rs`:
```rust
mod web_test;
```

Create `tests/tools/web_test.rs`:

```rust
// Test the html_to_markdown conversion function directly
use viv::tools::web::html_to_markdown;

#[test]
fn html_to_markdown_headings() {
    assert_eq!(html_to_markdown("<h1>Title</h1>"), "# Title\n\n");
    assert_eq!(html_to_markdown("<h2>Sub</h2>"), "## Sub\n\n");
}

#[test]
fn html_to_markdown_links() {
    assert_eq!(
        html_to_markdown(r#"<a href="https://example.com">Click</a>"#),
        "[Click](https://example.com)"
    );
}

#[test]
fn html_to_markdown_emphasis() {
    assert_eq!(html_to_markdown("<strong>bold</strong>"), "**bold**");
    assert_eq!(html_to_markdown("<em>italic</em>"), "*italic*");
}

#[test]
fn html_to_markdown_lists() {
    let html = "<ul><li>one</li><li>two</li></ul>";
    let md = html_to_markdown(html);
    assert!(md.contains("- one"));
    assert!(md.contains("- two"));
}

#[test]
fn html_to_markdown_code() {
    assert_eq!(html_to_markdown("<code>x + 1</code>"), "`x + 1`");
}

#[test]
fn html_to_markdown_paragraphs() {
    let md = html_to_markdown("<p>First</p><p>Second</p>");
    assert!(md.contains("First\n\n"));
    assert!(md.contains("Second"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test tools_tests web_test -- --nocapture`
Expected: compilation error — `html_to_markdown` not found.

- [ ] **Step 3: Implement html_to_markdown**

In `src/tools/web.rs`, replace `strip_html` with a public `html_to_markdown`. The implementation uses a single-pass state machine that:

```rust
pub fn html_to_markdown(html: &str) -> String {
    let mut out = String::new();
    let mut chars = html.chars().peekable();
    let mut in_tag = false;
    let mut tag_buf = String::new();
    let mut in_pre = false;

    while let Some(ch) = chars.next() {
        if ch == '<' {
            in_tag = true;
            tag_buf.clear();
            continue;
        }
        if in_tag {
            if ch == '>' {
                in_tag = false;
                let tag = tag_buf.trim().to_lowercase();
                handle_tag(&tag, &mut out, &mut in_pre);
            } else {
                tag_buf.push(ch);
            }
            continue;
        }
        // Decode common HTML entities
        if ch == '&' {
            let mut entity = String::new();
            for ec in chars.by_ref() {
                if ec == ';' { break; }
                entity.push(ec);
            }
            match entity.as_str() {
                "amp" => out.push('&'),
                "lt" => out.push('<'),
                "gt" => out.push('>'),
                "quot" => out.push('"'),
                "nbsp" => out.push(' '),
                _ => { out.push('&'); out.push_str(&entity); out.push(';'); }
            }
            continue;
        }
        if in_pre {
            out.push(ch);
        } else if ch == '\n' || ch == '\r' {
            // Collapse whitespace outside pre
            if !out.ends_with(' ') && !out.ends_with('\n') {
                out.push(' ');
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn handle_tag(tag: &str, out: &mut String, in_pre: &mut bool) {
    // Extract tag name (ignore attributes for closing tags)
    let (name, attrs) = if let Some(space) = tag.find(|c: char| c.is_whitespace()) {
        (&tag[..space], &tag[space..])
    } else {
        (tag, "")
    };

    match name {
        "h1" => out.push_str("# "),
        "/h1" => out.push_str("\n\n"),
        "h2" => out.push_str("## "),
        "/h2" => out.push_str("\n\n"),
        "h3" => out.push_str("### "),
        "/h3" => out.push_str("\n\n"),
        "h4" => out.push_str("#### "),
        "/h4" => out.push_str("\n\n"),
        "h5" => out.push_str("##### "),
        "/h5" => out.push_str("\n\n"),
        "h6" => out.push_str("###### "),
        "/h6" => out.push_str("\n\n"),
        "p" => {}
        "/p" => out.push_str("\n\n"),
        "br" | "br/" => out.push('\n'),
        "strong" | "b" => out.push_str("**"),
        "/strong" | "/b" => out.push_str("**"),
        "em" | "i" => out.push('*'),
        "/em" | "/i" => out.push('*'),
        "code" if !*in_pre => out.push('`'),
        "/code" if !*in_pre => out.push('`'),
        "pre" => { *in_pre = true; out.push_str("```\n"); }
        "/pre" => { *in_pre = false; out.push_str("\n```\n\n"); }
        "li" => out.push_str("- "),
        "/li" => out.push('\n'),
        "ul" | "/ul" | "ol" | "/ol" => out.push('\n'),
        _ if name.starts_with('a') => {
            // Extract href
            if let Some(href_start) = attrs.find("href=\"") {
                let rest = &attrs[href_start + 6..];
                if let Some(href_end) = rest.find('"') {
                    let href = &rest[..href_end];
                    out.push('[');
                    // The text content will be pushed by the main loop
                    // We need a way to close it — store href in out as marker
                    // Simple approach: push marker, close in /a
                    out.push_str(&format!("]({})", href));
                    // Actually, we need the text first. Let's use a different approach:
                    // For now, just push [  and store href to close later
                    // This simple parser can't do forward-looking, so:
                    // Push nothing here, handle in /a
                }
            }
        }
        _ => {}
    }
}
```

The key approach: for `<a>` tags, look ahead to find `</a>`, extract inner text, and build `[text](href)` format. For `<script>` and `<style>` tags, skip the entire content block.

```rust
pub fn html_to_markdown(html: &str) -> String {
    let mut out = String::new();
    let mut i = 0;
    let bytes = html.as_bytes();
    let len = bytes.len();
    let mut in_pre = false;

    while i < len {
        if bytes[i] == b'<' {
            let tag_start = i + 1;
            // Find closing >
            let tag_end = match html[tag_start..].find('>') {
                Some(p) => tag_start + p,
                None => { i += 1; continue; }
            };
            let tag_content = &html[tag_start..tag_end];
            let tag_lower = tag_content.to_lowercase();
            let tag_name = tag_lower.split_whitespace().next().unwrap_or("");

            match tag_name {
                "h1" => out.push_str("# "),
                "h2" => out.push_str("## "),
                "h3" => out.push_str("### "),
                "/h1" | "/h2" | "/h3" | "/h4" | "/h5" | "/h6" => out.push_str("\n\n"),
                "/p" => out.push_str("\n\n"),
                "br" | "br/" => out.push('\n'),
                "strong" | "b" | "/strong" | "/b" => out.push_str("**"),
                "em" | "i" | "/em" | "/i" => out.push('*'),
                "code" if !in_pre => out.push('`'),
                "/code" if !in_pre => out.push('`'),
                "pre" => { in_pre = true; out.push_str("```\n"); }
                "/pre" => { in_pre = false; out.push_str("\n```\n\n"); }
                "li" => out.push_str("- "),
                "/li" => out.push('\n'),
                "ul" | "/ul" | "ol" | "/ol" => out.push('\n'),
                a if a.starts_with('a') => {
                    // Extract href from attributes
                    if let Some(href) = extract_attr(tag_content, "href") {
                        out.push('[');
                        // Find closing </a>, collect inner text
                        let close = "</a>";
                        let inner_start = tag_end + 1;
                        if let Some(close_pos) = html[inner_start..].find(close) {
                            let inner = &html[inner_start..inner_start + close_pos];
                            // Strip any nested tags from inner
                            let text = strip_tags(inner);
                            out.push_str(&text);
                            out.push_str(&format!("]({})", href));
                            i = inner_start + close_pos + close.len();
                            continue;
                        }
                    }
                }
                "script" | "style" => {
                    // Skip script/style content entirely
                    let close = format!("</{}>", tag_name);
                    if let Some(p) = html[tag_end + 1..].find(&close) {
                        i = tag_end + 1 + p + close.len();
                        continue;
                    }
                }
                _ => {}
            }
            i = tag_end + 1;
        } else if bytes[i] == b'&' {
            let rest = &html[i + 1..];
            if let Some(end) = rest.find(';') {
                let entity = &rest[..end];
                match entity {
                    "amp" => out.push('&'),
                    "lt" => out.push('<'),
                    "gt" => out.push('>'),
                    "quot" => out.push('"'),
                    "nbsp" => out.push(' '),
                    _ => { out.push('&'); out.push_str(entity); out.push(';'); }
                }
                i += 2 + end;
            } else {
                out.push('&');
                i += 1;
            }
        } else {
            let ch = html[i..].chars().next().unwrap();
            if in_pre {
                out.push(ch);
            } else if ch == '\n' || ch == '\r' || ch == '\t' {
                if !out.ends_with(' ') && !out.ends_with('\n') {
                    out.push(' ');
                }
            } else {
                out.push(ch);
            }
            i += ch.len_utf8();
        }
    }
    out
}

fn extract_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let search = format!("{}=\"", name);
    let start = tag.to_lowercase().find(&search)?;
    let rest = &tag[start + search.len()..];
    let end = rest.find('"')?;
    Some(&rest[..end])
}

fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}
```

- [ ] **Step 4: Update truncation and usage in execute()**

In `src/tools/web.rs`, line 58, change:
```rust
            let truncated: String = text.chars().take(16000).collect();
```

And change `strip_html(&String::from_utf8_lossy(body))` to `html_to_markdown(&String::from_utf8_lossy(body))` in `fetch_url_async`.

- [ ] **Step 5: Run tests**

Run: `cargo test --test tools_tests web_test -- --nocapture`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/tools/web.rs tests/tools/web_test.rs tests/tools/mod.rs
git commit -m "feat: WebFetch converts HTML to Markdown, increases truncation to 16000"
```

---

## Task 12: NotebookEdit Tool

**Files:**
- Create: `src/tools/notebook.rs`
- Modify: `src/tools/mod.rs` (register + `pub mod notebook`)
- Create: `tests/tools/notebook_test.rs`
- Modify: `tests/tools/mod.rs`

- [ ] **Step 1: Write failing tests**

Create `tests/tools/notebook_test.rs`:

```rust
use std::fs;
use viv::core::json::JsonValue;
use viv::tools::Tool;
use viv::tools::notebook::NotebookEditTool;
use viv::tools::poll_to_completion;

fn tempdir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("viv_nb_{}", nanos()));
    fs::create_dir_all(&p).unwrap();
    p
}
fn nanos() -> u32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos()
}
fn json_path(p: &std::path::Path) -> String {
    p.display().to_string().replace('\\', "\\\\")
}

const SAMPLE_NOTEBOOK: &str = r#"{
 "cells": [
  {"cell_type": "code", "source": ["print('hello')\n"], "metadata": {}, "outputs": [], "id": "cell1"},
  {"cell_type": "markdown", "source": ["# Title\n"], "metadata": {}, "id": "cell2"}
 ],
 "metadata": {},
 "nbformat": 4,
 "nbformat_minor": 5
}"#;

#[test]
fn notebook_replace_cell_by_id() {
    let dir = tempdir();
    let path = dir.join("test.ipynb");
    fs::write(&path, SAMPLE_NOTEBOOK).unwrap();

    let input = JsonValue::parse(&format!(
        r#"{{"notebook_path":"{}","cell_id":"cell1","new_source":"print('world')"}}"#,
        json_path(&path)
    )).unwrap();
    let result = poll_to_completion(NotebookEditTool.execute(&input)).unwrap();
    assert!(result.contains("Replaced"));

    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("world"));
    assert!(!content.contains("hello"));
}

#[test]
fn notebook_insert_cell() {
    let dir = tempdir();
    let path = dir.join("test.ipynb");
    fs::write(&path, SAMPLE_NOTEBOOK).unwrap();

    let input = JsonValue::parse(&format!(
        r#"{{"notebook_path":"{}","cell_id":"cell1","edit_mode":"insert","cell_type":"code","new_source":"x = 42"}}"#,
        json_path(&path)
    )).unwrap();
    let result = poll_to_completion(NotebookEditTool.execute(&input)).unwrap();
    assert!(result.contains("Inserted"));

    let content = fs::read_to_string(&path).unwrap();
    let parsed = JsonValue::parse(&content).unwrap();
    let cells = parsed.get("cells").unwrap().as_array().unwrap();
    assert_eq!(cells.len(), 3);
}

#[test]
fn notebook_delete_cell() {
    let dir = tempdir();
    let path = dir.join("test.ipynb");
    fs::write(&path, SAMPLE_NOTEBOOK).unwrap();

    let input = JsonValue::parse(&format!(
        r#"{{"notebook_path":"{}","cell_id":"cell2","edit_mode":"delete","new_source":""}}"#,
        json_path(&path)
    )).unwrap();
    let result = poll_to_completion(NotebookEditTool.execute(&input)).unwrap();
    assert!(result.contains("Deleted"));

    let content = fs::read_to_string(&path).unwrap();
    let parsed = JsonValue::parse(&content).unwrap();
    let cells = parsed.get("cells").unwrap().as_array().unwrap();
    assert_eq!(cells.len(), 1);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test tools_tests notebook_test -- --nocapture`
Expected: compilation error — module not found.

- [ ] **Step 3: Implement NotebookEditTool**

Create `src/tools/notebook.rs` with the full implementation. (The tool parses ipynb JSON, locates cell by `id` field, performs replace/insert/delete, writes back.) The implementation uses `JsonValue` for parsing and manipulation, modifying the `cells` array in-place.

Key implementation logic:
- Parse notebook JSON via `JsonValue::parse`
- Find cell index matching `cell_id`
- For replace: update `source` field to `["line1\n", "line2\n", ...]`
- For insert: construct new cell object and insert at index + 1
- For delete: remove cell at index
- Serialize back with `format!("{}", notebook_json)`

- [ ] **Step 4: Register module**

In `src/tools/mod.rs`, add `pub mod notebook;` and register in `default_tools()`:

```rust
use crate::tools::notebook::NotebookEditTool;
// ...
reg.register(Box::new(NotebookEditTool));
```

In `tests/tools/mod.rs`, add `mod notebook_test;`.

- [ ] **Step 5: Run tests**

Run: `cargo test --test tools_tests notebook_test -- --nocapture`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/tools/notebook.rs src/tools/mod.rs tests/tools/notebook_test.rs tests/tools/mod.rs
git commit -m "feat: add NotebookEdit tool for Jupyter notebook cell editing"
```

---

## Task 13: WebSearch Tool (Tavily)

**Files:**
- Create: `src/tools/search.rs`
- Modify: `src/tools/mod.rs` (register + `pub mod search`)
- Create: `tests/tools/search_test.rs`

- [ ] **Step 1: Write test for missing API key error**

Create `tests/tools/search_test.rs`:

```rust
use viv::core::json::JsonValue;
use viv::tools::Tool;
use viv::tools::search::WebSearchTool;
use viv::tools::poll_to_completion;

#[test]
fn search_without_api_key_returns_friendly_error() {
    // Ensure VIV_TAVILY_API_KEY is not set for this test
    std::env::remove_var("VIV_TAVILY_API_KEY");
    let tool = WebSearchTool;
    let input = JsonValue::parse(r#"{"query":"rust programming"}"#).unwrap();
    let result = poll_to_completion(tool.execute(&input));
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("VIV_TAVILY_API_KEY"), "Error should mention env var: {}", err);
}

#[test]
fn search_tool_has_correct_name_and_permission() {
    let tool = WebSearchTool;
    assert_eq!(tool.name(), "WebSearch");
    assert_eq!(tool.permission_level(), viv::tools::PermissionLevel::ReadOnly);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test tools_tests search_test -- --nocapture`
Expected: compilation error — module not found.

- [ ] **Step 3: Implement WebSearchTool**

Create `src/tools/search.rs`:

```rust
use crate::core::json::JsonValue;
use crate::core::net::http::HttpRequest;
use crate::core::net::tls::AsyncTlsStream;
use crate::core::runtime::AssertSend;
use crate::error::Error;
use crate::tools::{PermissionLevel, Tool};
use std::future::Future;
use std::pin::Pin;

pub struct WebSearchTool;

impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "WebSearch"
    }

    fn description(&self) -> &str {
        "Search the web for real-time information using Tavily's AI search engine.\n\n- Returns relevant web content with titles, URLs, and content snippets\n- Requires VIV_TAVILY_API_KEY environment variable\n- Use for accessing information beyond the model's knowledge cutoff\n- search_depth: 'basic' (fast) or 'advanced' (thorough)\n- topic: 'general' or 'news'"
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "query":{"type":"string","description":"Search query"},
                "max_results":{"type":"number","description":"Maximum number of results (default 10, max 20)"},
                "search_depth":{"type":"string","description":"basic or advanced (default basic)"},
                "topic":{"type":"string","description":"general or news (default general)"},
                "include_domains":{"type":"array","items":{"type":"string"},"description":"Only include results from these domains"},
                "exclude_domains":{"type":"array","items":{"type":"string"},"description":"Exclude results from these domains"}
            },
            "required":["query"]
        }"#).unwrap()
    }

    fn execute(
        &self,
        input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let input = input.clone();
        Box::pin(AssertSend(async move {
            let api_key = std::env::var("VIV_TAVILY_API_KEY")
                .map_err(|_| Error::Tool("VIV_TAVILY_API_KEY not set. Get a key at https://tavily.com".into()))?;

            let query = input.get("query").and_then(|v| v.as_str())
                .ok_or_else(|| Error::Tool("missing 'query'".into()))?;
            let max_results = input.get("max_results").and_then(|v| v.as_i64()).unwrap_or(10).min(20);
            let depth = input.get("search_depth").and_then(|v| v.as_str()).unwrap_or("basic");
            let topic = input.get("topic").and_then(|v| v.as_str()).unwrap_or("general");

            // Build request body
            let mut body_parts = vec![
                format!("\"api_key\":{}", JsonValue::Str(api_key)),
                format!("\"query\":{}", JsonValue::Str(query.into())),
                format!("\"max_results\":{}", max_results),
                format!("\"search_depth\":{}", JsonValue::Str(depth.into())),
                format!("\"topic\":{}", JsonValue::Str(topic.into())),
            ];

            if let Some(inc) = input.get("include_domains").and_then(|v| v.as_array()) {
                let domains: Vec<String> = inc.iter()
                    .filter_map(|v| v.as_str().map(|s| format!("{}", JsonValue::Str(s.into()))))
                    .collect();
                body_parts.push(format!("\"include_domains\":[{}]", domains.join(",")));
            }
            if let Some(exc) = input.get("exclude_domains").and_then(|v| v.as_array()) {
                let domains: Vec<String> = exc.iter()
                    .filter_map(|v| v.as_str().map(|s| format!("{}", JsonValue::Str(s.into()))))
                    .collect();
                body_parts.push(format!("\"exclude_domains\":[{}]", domains.join(",")));
            }

            let body = format!("{{{}}}", body_parts.join(","));

            let req = HttpRequest {
                method: "POST".into(),
                path: "/search".into(),
                headers: vec![
                    ("Host".into(), "api.tavily.com".into()),
                    ("Content-Type".into(), "application/json".into()),
                    ("Content-Length".into(), body.len().to_string()),
                    ("Connection".into(), "close".into()),
                ],
                body: Some(body),
            };

            let mut tls = AsyncTlsStream::connect("api.tavily.com", 443).await?;
            tls.write_all(&req.to_bytes()).await?;

            let mut raw: Vec<u8> = Vec::new();
            let mut tmp = [0u8; 4096];
            loop {
                let n = tls.read(&mut tmp).await?;
                if n == 0 || raw.len() > 500_000 { break; }
                raw.extend_from_slice(&tmp[..n]);
            }

            let resp_body = raw.windows(4)
                .position(|w| w == b"\r\n\r\n")
                .map(|i| &raw[i + 4..])
                .unwrap_or(&raw);

            let json = JsonValue::parse(&String::from_utf8_lossy(resp_body))
                .map_err(|e| Error::Tool(format!("failed to parse Tavily response: {}", e)))?;

            // Format results
            let results = json.get("results").and_then(|v| v.as_array())
                .ok_or_else(|| Error::Tool("no results in Tavily response".into()))?;

            let mut out = String::new();
            for (i, r) in results.iter().enumerate() {
                let title = r.get("title").and_then(|v| v.as_str()).unwrap_or("(no title)");
                let url = r.get("url").and_then(|v| v.as_str()).unwrap_or("");
                let content = r.get("content").and_then(|v| v.as_str()).unwrap_or("");
                out.push_str(&format!("{}. {}\n   URL: {}\n   {}\n\n", i + 1, title, url, content));
            }

            Ok(out)
        }))
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
}
```

- [ ] **Step 4: Register module**

In `src/tools/mod.rs`, add `pub mod search;` and register:

```rust
use crate::tools::search::WebSearchTool;
// ...
reg.register(Box::new(WebSearchTool));
```

In `tests/tools/mod.rs`, add `mod search_test;`.

- [ ] **Step 5: Run tests**

Run: `cargo test --test tools_tests search_test -- --nocapture`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/tools/search.rs src/tools/mod.rs tests/tools/search_test.rs tests/tools/mod.rs
git commit -m "feat: add WebSearch tool using Tavily API"
```

---

## Task 14: Runtime Combinators (join, join_all)

**Files:**
- Modify: `src/core/runtime/mod.rs`
- Create: `tests/core/runtime/join_test.rs`
- Modify: `tests/core/runtime/mod.rs`

- [ ] **Step 1: Write failing tests**

Create `tests/core/runtime/join_test.rs`:

```rust
use viv::core::runtime::{join, join_all, block_on_local};

#[test]
fn join_two_futures() {
    let result = block_on_local(Box::pin(async {
        join(
            async { 1 + 1 },
            async { "hello" },
        ).await
    }));
    assert_eq!(result, (2, "hello"));
}

#[test]
fn join_all_multiple_futures() {
    let result = block_on_local(Box::pin(async {
        let futures: Vec<_> = (0..5).map(|i| async move { i * 2 }).collect();
        join_all(futures).await
    }));
    assert_eq!(result, vec![0, 2, 4, 6, 8]);
}

#[test]
fn join_all_empty_vec() {
    let result: Vec<i32> = block_on_local(Box::pin(async {
        let futures: Vec<std::pin::Pin<Box<dyn std::future::Future<Output = i32> + Send>>> = vec![];
        join_all(futures).await
    }));
    assert!(result.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test core_tests join_test -- --nocapture`
Expected: compilation error — `join` and `join_all` not found.

- [ ] **Step 3: Implement join and join_all**

Add to `src/core/runtime/mod.rs`:

```rust
pub use combinator::{join, join_all};

pub mod combinator;
```

Create `src/core/runtime/combinator.rs`:

```rust
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Run two futures concurrently, return both results.
pub async fn join<A, B>(a: A, b: B) -> (A::Output, B::Output)
where
    A: Future,
    B: Future,
{
    JoinTwo { a: MaybeDone::Pending(a), b: MaybeDone::Pending(b) }.await
}

enum MaybeDone<F: Future> {
    Pending(F),
    Done(F::Output),
    Taken,
}

struct JoinTwo<A: Future, B: Future> {
    a: MaybeDone<A>,
    b: MaybeDone<B>,
}

impl<A: Future + Unpin, B: Future + Unpin> Future for JoinTwo<A, B>
where
    A::Output: Unpin,
    B::Output: Unpin,
{
    type Output = (A::Output, B::Output);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        if let MaybeDone::Pending(ref mut f) = this.a {
            if let Poll::Ready(val) = Pin::new(f).poll(cx) {
                this.a = MaybeDone::Done(val);
            }
        }
        if let MaybeDone::Pending(ref mut f) = this.b {
            if let Poll::Ready(val) = Pin::new(f).poll(cx) {
                this.b = MaybeDone::Done(val);
            }
        }

        match (&mut this.a, &mut this.b) {
            (MaybeDone::Done(_), MaybeDone::Done(_)) => {
                let a = std::mem::replace(&mut this.a, MaybeDone::Taken);
                let b = std::mem::replace(&mut this.b, MaybeDone::Taken);
                if let (MaybeDone::Done(a), MaybeDone::Done(b)) = (a, b) {
                    Poll::Ready((a, b))
                } else {
                    unreachable!()
                }
            }
            _ => Poll::Pending,
        }
    }
}

/// Run a vec of futures concurrently, return all results in order.
pub async fn join_all<F>(futures: Vec<F>) -> Vec<F::Output>
where
    F: Future + Unpin,
    F::Output: Unpin,
{
    JoinAll {
        states: futures.into_iter().map(|f| MaybeDone::Pending(f)).collect(),
    }.await
}

struct JoinAll<F: Future> {
    states: Vec<MaybeDone<F>>,
}

impl<F: Future + Unpin> Future for JoinAll<F>
where
    F::Output: Unpin,
{
    type Output = Vec<F::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut all_done = true;

        for state in this.states.iter_mut() {
            if let MaybeDone::Pending(ref mut f) = state {
                match Pin::new(f).poll(cx) {
                    Poll::Ready(val) => *state = MaybeDone::Done(val),
                    Poll::Pending => all_done = false,
                }
            }
        }

        if all_done {
            let results: Vec<_> = this.states.iter_mut().map(|s| {
                match std::mem::replace(s, MaybeDone::Taken) {
                    MaybeDone::Done(v) => v,
                    _ => unreachable!(),
                }
            }).collect();
            Poll::Ready(results)
        } else {
            Poll::Pending
        }
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test core_tests join_test -- --nocapture`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/core/runtime/combinator.rs src/core/runtime/mod.rs tests/core/runtime/join_test.rs tests/core/runtime/mod.rs
git commit -m "feat: add join/join_all async combinators"
```

---

## Task 15: SubAgent Tool

**Files:**
- Create: `src/tools/agent.rs`
- Modify: `src/tools/mod.rs` (register + `pub mod agent`)
- Modify: `src/agent/agent.rs` (add `new_sub()`)
- Create: `tests/tools/agent_test.rs`

- [ ] **Step 1: Write test for SubAgentTool structure**

Create `tests/tools/agent_test.rs`:

```rust
use viv::tools::Tool;
use viv::tools::agent::SubAgentTool;

#[test]
fn sub_agent_tool_has_correct_name_and_permission() {
    let llm_config = viv::llm::LLMConfig::from_env();
    if llm_config.is_err() { return; }
    let llm = std::sync::Arc::new(viv::llm::LLMClient::new(llm_config.unwrap()));
    let tool = SubAgentTool::new(llm);
    assert_eq!(tool.name(), "Agent");
    assert_eq!(tool.permission_level(), viv::tools::PermissionLevel::ReadOnly);
}

#[test]
fn sub_agent_schema_has_prompt_required() {
    let llm_config = viv::llm::LLMConfig::from_env();
    if llm_config.is_err() { return; }
    let llm = std::sync::Arc::new(viv::llm::LLMClient::new(llm_config.unwrap()));
    let tool = SubAgentTool::new(llm);
    let schema = tool.input_schema();
    let required = schema.get("required").unwrap().as_array().unwrap();
    assert!(required.iter().any(|v| v.as_str() == Some("prompt")));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test tools_tests agent_test -- --nocapture`
Expected: compilation error.

- [ ] **Step 3: Implement SubAgentTool**

Create `src/tools/agent.rs` with the SubAgentTool struct implementing the Tool trait. The `execute()` method:
1. Parses prompt, model tier, max_iterations from input
2. Creates `agent_channel()`
3. Calls `Agent::new_sub()` with the endpoint
4. Runs `join(child.run(), monitor_loop)` concurrently
5. Monitor loop collects TextChunk messages, handles PermissionRequest, exits on Done

- [ ] **Step 4: Add Agent::new_sub()**

Add to `src/agent/agent.rs`:

```rust
    /// Create a lightweight sub-agent for executing a delegated task.
    /// No MCP/LSP/Memory — just LLM + tools.
    pub async fn new_sub(
        config: AgentConfig,
        endpoint: crate::bus::channel::AgentEndpoint,
        llm: std::sync::Arc<crate::llm::LLMClient>,
    ) -> Result<Self> {
        let tools = ToolRegistry::default_tools_without("Agent", std::sync::Arc::clone(&llm));
        let store = std::sync::Arc::new(MemoryStore::new(config.memory_dir.clone())?);
        let index = std::sync::Arc::new(std::sync::Mutex::new(MemoryIndex::default()));
        let mcp = std::sync::Arc::new(std::sync::Mutex::new(McpManager::empty()));
        let lsp = std::sync::Arc::new(std::sync::Mutex::new(LspManager::empty()));

        Ok(Agent {
            messages: vec![],
            prompt_cache: PromptCache::default(),
            llm,
            store,
            index,
            tools,
            permissions: PermissionManager::default(),
            config,
            input_tokens: 0,
            output_tokens: 0,
            event_rx: endpoint.rx,
            msg_tx: endpoint.tx,
            mcp,
            lsp,
        })
    }
```

Note: `McpManager::empty()` and `LspManager::empty()` may need to be added if they don't exist. They return a manager with no servers configured.

- [ ] **Step 5: Register module**

In `src/tools/mod.rs`, add `pub mod agent;` and register in `default_tools()`:

```rust
use crate::tools::agent::SubAgentTool;
// ...
reg.register(Box::new(SubAgentTool::new(Arc::clone(&llm))));
```

In `tests/tools/mod.rs`, add `mod agent_test;`.

- [ ] **Step 6: Run tests**

Run: `cargo test --test tools_tests agent_test -- --nocapture`
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/tools/agent.rs src/tools/mod.rs src/agent/agent.rs tests/tools/agent_test.rs tests/tools/mod.rs
git commit -m "feat: add SubAgent tool with agent_channel communication"
```

---

## Task 16: Agent Loop — Concurrent SubAgent Execution

**Files:**
- Modify: `src/agent/agent.rs:272-340` (agentic_loop)

- [ ] **Step 1: Write test verifying Agent tool is partitioned**

This is best tested via integration test or by verifying the agentic_loop behavior. For unit testing, add to `tests/tools/agent_test.rs`:

```rust
#[test]
fn default_tools_includes_agent_tool() {
    let llm_config = viv::llm::LLMConfig::from_env();
    if llm_config.is_err() { return; }
    let llm = std::sync::Arc::new(viv::llm::LLMClient::new(llm_config.unwrap()));
    let reg = viv::tools::ToolRegistry::default_tools(llm);
    assert!(reg.get("Agent").is_some(), "Agent tool should be registered");
}

#[test]
fn default_tools_without_agent_excludes_it() {
    let llm_config = viv::llm::LLMConfig::from_env();
    if llm_config.is_err() { return; }
    let llm = std::sync::Arc::new(viv::llm::LLMClient::new(llm_config.unwrap()));
    let reg = viv::tools::ToolRegistry::default_tools_without("Agent", llm);
    assert!(reg.get("Agent").is_none(), "Agent tool should be excluded");
}
```

- [ ] **Step 2: Modify agentic_loop for concurrent Agent execution**

In `src/agent/agent.rs`, replace the tool execution section (lines ~275-317) with partitioned execution:

```rust
            // Partition: Agent tools run concurrently, others run serially
            let mut agent_tool_uses = Vec::new();
            let mut normal_tool_uses = Vec::new();
            for tu in &tool_uses {
                if let ContentBlock::ToolUse { name, .. } = tu {
                    if name == "Agent" {
                        agent_tool_uses.push(tu);
                    } else {
                        normal_tool_uses.push(tu);
                    }
                }
            }

            // Serial execution for normal tools
            for tu in &normal_tool_uses {
                // ... existing execution logic (permission check, execute, collect result)
            }

            // Concurrent execution for Agent tools
            if !agent_tool_uses.is_empty() {
                use crate::core::runtime::join_all;
                let futures: Vec<_> = agent_tool_uses.iter().map(|tu| {
                    if let ContentBlock::ToolUse { id, name, input } = tu {
                        let tool = self.tools.get(name);
                        // ... build future
                    }
                }).collect();
                let results = join_all(futures).await;
                tool_results.extend(results);
            }
```

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add src/agent/agent.rs tests/tools/agent_test.rs
git commit -m "feat: agent loop partitions Agent tool calls for concurrent execution"
```

---

## Task 17: Final Integration — cargo test + cargo clippy

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy`
Expected: no errors (warnings OK).

- [ ] **Step 3: Run fmt**

Run: `cargo fmt`

- [ ] **Step 4: Final commit if any formatting changes**

```bash
git add -A
git commit -m "chore: format code"
```
