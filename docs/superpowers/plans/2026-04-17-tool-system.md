# Tool System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the tool stub in the agent loop with 12 real built-in tools, a tiered permission model, and Anthropic JSON schema integration.

**Architecture:** `Tool` trait + static `ToolRegistry` live in `src/tools/`. `PermissionManager` in `src/permissions/` gates Write/Execute tools with an ask-then-remember policy. `AgentContext` gains these two fields. `run_agent` dispatches real tool calls. The LLM request body includes an Anthropic `tools` array built from the registry.

**Tech Stack:** Rust std only (zero external deps). Shell via `std::process::Command`. File I/O via `std::fs`. WebFetch reuses `src/net/tls.rs` + `src/net/http.rs`.

---

## Files Created / Modified

**Created:**
- `src/tools/mod.rs` — Tool trait, PermissionLevel, ToolRegistry
- `src/tools/bash.rs` — BashTool, BashBackgroundTool
- `src/tools/todo.rs` — TodoWriteTool, TodoReadTool
- `src/tools/web.rs` — WebFetchTool
- `src/tools/file/mod.rs` — file submodule
- `src/tools/file/read.rs` — ReadTool
- `src/tools/file/write.rs` — WriteTool
- `src/tools/file/edit.rs` — EditTool, MultiEditTool
- `src/tools/file/glob.rs` — GlobTool
- `src/tools/file/grep.rs` — GrepTool
- `src/tools/file/ls.rs` — LsTool
- `src/permissions/mod.rs` — re-exports
- `src/permissions/manager.rs` — PermissionManager
- `tests/tools/mod.rs`, `tests/tools/registry_test.rs`
- `tests/tools/bash_test.rs`, `tests/tools/todo_test.rs`
- `tests/tools/file/mod.rs`, `tests/tools/file/read_test.rs`
- `tests/tools/file/write_test.rs`, `tests/tools/file/edit_test.rs`
- `tests/tools/file/glob_test.rs`, `tests/tools/file/grep_test.rs`
- `tests/tools/file/ls_test.rs`
- `tests/permissions/mod.rs`, `tests/permissions/manager_test.rs`

**Modified:**
- `src/error.rs` — add `Error::Tool(String)`
- `src/lib.rs` — add `pub mod tools; pub mod permissions;`
- `src/llm.rs` — `stream_agent` + `build_agent_request` gain `tools_json: &str`
- `src/memory/retrieval.rs:67` — pass `""` to stream_agent
- `src/memory/compaction.rs:31` — pass `""` to stream_agent
- `src/agent/evolution.rs:41,110` — pass `""` to stream_agent
- `src/agent/context.rs` — add `tool_registry`, `permission_manager`
- `src/agent/run.rs` — new signature, real tool dispatch
- `src/repl.rs` — inject `ask_fn`, add `JsonValue` import

---

### Task 1: Error::Tool + Tool trait + ToolRegistry

**Files:**
- Modify: `src/error.rs`
- Modify: `src/lib.rs`
- Create: `src/tools/mod.rs`
- Create: `src/tools/bash.rs` (stub)
- Create: `src/tools/todo.rs` (stub)
- Create: `src/tools/web.rs` (stub)
- Create: `src/tools/file/mod.rs`
- Create: `src/tools/file/read.rs` (stub)
- Create: `src/tools/file/write.rs` (stub)
- Create: `src/tools/file/edit.rs` (stub)
- Create: `src/tools/file/glob.rs` (stub)
- Create: `src/tools/file/grep.rs` (stub)
- Create: `src/tools/file/ls.rs` (stub)
- Create: `src/permissions/mod.rs`
- Create: `src/permissions/manager.rs` (stub)
- Create: `tests/tools/mod.rs`
- Create: `tests/tools/registry_test.rs`

- [ ] **Step 1: Write the failing tests**

Create `tests/tools/mod.rs`:
```rust
```

Create `tests/tools/registry_test.rs`:
```rust
use viv::json::JsonValue;
use viv::tools::{PermissionLevel, Tool, ToolRegistry};

struct EchoTool;

impl Tool for EchoTool {
    fn name(&self) -> &str { "echo" }
    fn description(&self) -> &str { "Echoes input text" }
    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}"#).unwrap()
    }
    fn execute(&self, input: &JsonValue) -> viv::Result<String> {
        Ok(input.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string())
    }
    fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }
}

#[test]
fn registry_get_returns_registered_tool() {
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(EchoTool));
    assert!(reg.get("echo").is_some());
    assert!(reg.get("missing").is_none());
}

#[test]
fn registry_to_api_json_has_required_fields() {
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(EchoTool));
    let json = reg.to_api_json();
    assert!(json.contains("\"name\":\"echo\""));
    assert!(json.contains("\"description\""));
    assert!(json.contains("\"input_schema\""));
}

#[test]
fn tool_execute_returns_input_text() {
    let input = JsonValue::parse(r#"{"text":"hello"}"#).unwrap();
    assert_eq!(EchoTool.execute(&input).unwrap(), "hello");
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test --test registry_test 2>&1 | head -15
```
Expected: FAIL — `viv::tools` not found.

- [ ] **Step 3: Add `Error::Tool` to `src/error.rs`**

Full file after change:
```rust
use std::fmt;

#[derive(Debug)]
pub enum Error {
    Json(String),
    Terminal(String),
    Io(std::io::Error),
    Tls(String),
    Http(String),
    LLM { status: u16, message: String },
    Tool(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Json(msg) => write!(f, "JSON error: {}", msg),
            Error::Terminal(msg) => write!(f, "terminal error: {}", msg),
            Error::Io(err) => write!(f, "IO error: {}", err),
            Error::Tls(msg) => write!(f, "TLS error: {}", msg),
            Error::Http(msg) => write!(f, "HTTP error: {}", msg),
            Error::LLM { status, message } => write!(f, "LLM error {}: {}", status, message),
            Error::Tool(msg) => write!(f, "tool error: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self { Error::Io(err) }
}
```

- [ ] **Step 4: Add module declarations to `src/lib.rs`**

Insert two lines before `pub mod llm;`:
```rust
pub mod agent;
pub mod memory;
pub mod permissions;
pub mod runtime;
pub mod tools;
pub mod llm;
pub mod error;
pub mod event;
pub mod json;
pub mod net;
pub mod repl;
pub mod terminal;
pub mod tui;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;
```

- [ ] **Step 5: Create `src/tools/mod.rs`**

```rust
use crate::json::JsonValue;

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> JsonValue;
    fn execute(&self, input: &JsonValue) -> crate::Result<String>;
    fn permission_level(&self) -> PermissionLevel;
}

#[derive(Debug, Clone, PartialEq)]
pub enum PermissionLevel {
    ReadOnly,
    Write,
    Execute,
}

pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        ToolRegistry { tools: vec![] }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.iter().find(|t| t.name() == name).map(|t| t.as_ref())
    }

    pub fn to_api_json(&self) -> String {
        let tools: Vec<String> = self.tools.iter().map(|t| {
            format!(
                "{{\"name\":{},\"description\":{},\"input_schema\":{}}}",
                JsonValue::Str(t.name().into()),
                JsonValue::Str(t.description().into()),
                t.input_schema(),
            )
        }).collect();
        format!("[{}]", tools.join(","))
    }

    pub fn default_tools(llm: std::sync::Arc<crate::llm::LLMClient>) -> Self {
        let _ = llm; // used in Task 8 when WebFetchTool is registered
        ToolRegistry::new()
    }
}

pub mod bash;
pub mod file;
pub mod todo;
pub mod web;
```

- [ ] **Step 6: Create stub files for all submodules**

Create `src/tools/bash.rs`:
```rust
// implemented in Task 5
```

Create `src/tools/todo.rs`:
```rust
// implemented in Task 6
```

Create `src/tools/web.rs`:
```rust
// implemented in Task 6
```

Create `src/tools/file/mod.rs`:
```rust
pub mod edit;
pub mod glob;
pub mod grep;
pub mod ls;
pub mod read;
pub mod write;
```

Create `src/tools/file/read.rs`, `src/tools/file/write.rs`, `src/tools/file/edit.rs`, `src/tools/file/glob.rs`, `src/tools/file/grep.rs`, `src/tools/file/ls.rs` — each containing just `// stub`.

Create `src/permissions/mod.rs`:
```rust
pub mod manager;
pub use manager::PermissionManager;
```

Create `src/permissions/manager.rs`:
```rust
// implemented in Task 2
```

- [ ] **Step 7: Run tests — verify they pass**

```bash
cargo test --test registry_test 2>&1
```
Expected: `test result: ok. 3 passed`

- [ ] **Step 8: Commit**

```bash
git add src/error.rs src/lib.rs src/tools/ src/permissions/ tests/tools/
git commit -m "feat(tools): Tool trait + ToolRegistry + Error::Tool foundation"
```

---

### Task 2: PermissionManager

**Files:**
- Modify: `src/permissions/manager.rs`
- Create: `tests/permissions/mod.rs`
- Create: `tests/permissions/manager_test.rs`

- [ ] **Step 1: Write the failing tests**

Create `tests/permissions/mod.rs`:
```rust
```

Create `tests/permissions/manager_test.rs`:
```rust
use viv::json::JsonValue;
use viv::permissions::PermissionManager;
use viv::tools::{PermissionLevel, Tool};

struct FakeTool { level: PermissionLevel }

impl Tool for FakeTool {
    fn name(&self) -> &str { "fake" }
    fn description(&self) -> &str { "" }
    fn input_schema(&self) -> JsonValue { JsonValue::Null }
    fn execute(&self, _: &JsonValue) -> viv::Result<String> { Ok("ok".into()) }
    fn permission_level(&self) -> PermissionLevel { self.level.clone() }
}

#[test]
fn readonly_always_allowed_without_asking() {
    let mut pm = PermissionManager::default();
    let tool = FakeTool { level: PermissionLevel::ReadOnly };
    let mut asked = false;
    let allowed = pm.check(&tool, &JsonValue::Null, &mut |_, _| { asked = true; true });
    assert!(allowed);
    assert!(!asked);
}

#[test]
fn write_asks_on_first_call() {
    let mut pm = PermissionManager::default();
    let tool = FakeTool { level: PermissionLevel::Write };
    let mut asked = false;
    let allowed = pm.check(&tool, &JsonValue::Null, &mut |_, _| { asked = true; true });
    assert!(asked);
    assert!(allowed);
}

#[test]
fn write_not_asked_again_after_grant() {
    let mut pm = PermissionManager::default();
    let tool = FakeTool { level: PermissionLevel::Write };
    pm.check(&tool, &JsonValue::Null, &mut |_, _| true);
    let mut asked_again = false;
    let allowed = pm.check(&tool, &JsonValue::Null, &mut |_, _| { asked_again = true; false });
    assert!(!asked_again);
    assert!(allowed);
}

#[test]
fn denied_not_remembered_next_call_asks_again() {
    let mut pm = PermissionManager::default();
    let tool = FakeTool { level: PermissionLevel::Execute };
    pm.check(&tool, &JsonValue::Null, &mut |_, _| false);
    let mut asked_again = false;
    pm.check(&tool, &JsonValue::Null, &mut |_, _| { asked_again = true; false });
    assert!(asked_again);
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test --test manager_test 2>&1 | head -10
```
Expected: FAIL — `PermissionManager` not implemented.

- [ ] **Step 3: Implement `src/permissions/manager.rs`**

```rust
use std::collections::HashSet;
use crate::json::JsonValue;
use crate::tools::{Tool, PermissionLevel};

#[derive(Default)]
pub struct PermissionManager {
    session_allowed: HashSet<String>,
}

impl PermissionManager {
    pub fn check(
        &mut self,
        tool: &dyn Tool,
        input: &JsonValue,
        ask_fn: &mut dyn FnMut(&str, &JsonValue) -> bool,
    ) -> bool {
        if tool.permission_level() == PermissionLevel::ReadOnly {
            return true;
        }
        if self.session_allowed.contains(tool.name()) {
            return true;
        }
        let granted = ask_fn(tool.name(), input);
        if granted {
            self.session_allowed.insert(tool.name().to_string());
        }
        granted
    }
}
```

- [ ] **Step 4: Run tests — verify they pass**

```bash
cargo test --test manager_test 2>&1
```
Expected: `test result: ok. 4 passed`

- [ ] **Step 5: Commit**

```bash
git add src/permissions/manager.rs tests/permissions/
git commit -m "feat(permissions): PermissionManager — ask-then-remember per session"
```

---

### Task 3: ReadOnly file tools — Read, LS, Glob

**Files:**
- Modify: `src/tools/file/read.rs`
- Modify: `src/tools/file/ls.rs`
- Modify: `src/tools/file/glob.rs`
- Create: `tests/tools/file/mod.rs`
- Create: `tests/tools/file/read_test.rs`
- Create: `tests/tools/file/ls_test.rs`
- Create: `tests/tools/file/glob_test.rs`

- [ ] **Step 1: Write the failing tests**

Create `tests/tools/file/mod.rs`:
```rust
```

Create `tests/tools/file/read_test.rs`:
```rust
use std::fs;
use viv::json::JsonValue;
use viv::tools::Tool;
use viv::tools::file::read::ReadTool;

fn tempdir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("viv_read_{}", nanos()));
    fs::create_dir_all(&p).unwrap();
    p
}
fn nanos() -> u32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos()
}

#[test]
fn read_returns_content_with_line_numbers() {
    let dir = tempdir();
    let path = dir.join("f.txt");
    fs::write(&path, "alpha\nbeta\ngamma\n").unwrap();
    let input = JsonValue::parse(&format!(r#"{{"file_path":"{}"}}"#, path.display())).unwrap();
    let result = ReadTool.execute(&input).unwrap();
    assert!(result.contains("alpha"));
    assert!(result.contains("beta"));
    assert!(result.contains('1'));  // line number
}

#[test]
fn read_with_offset_skips_lines() {
    let dir = tempdir();
    let path = dir.join("f.txt");
    fs::write(&path, "a\nb\nc\n").unwrap();
    let input = JsonValue::parse(&format!(r#"{{"file_path":"{}","offset":2,"limit":1}}"#, path.display())).unwrap();
    let result = ReadTool.execute(&input).unwrap();
    assert!(result.contains('b'));
    assert!(!result.contains('a'));
    assert!(!result.contains('c'));
}

#[test]
fn read_missing_file_is_error() {
    let input = JsonValue::parse(r#"{"file_path":"/nonexistent/no.txt"}"#).unwrap();
    assert!(ReadTool.execute(&input).is_err());
}
```

Create `tests/tools/file/ls_test.rs`:
```rust
use std::fs;
use viv::json::JsonValue;
use viv::tools::Tool;
use viv::tools::file::ls::LsTool;

fn tempdir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("viv_ls_{}", nanos()));
    fs::create_dir_all(&p).unwrap();
    p
}
fn nanos() -> u32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos()
}

#[test]
fn ls_shows_files_in_directory() {
    let dir = tempdir();
    fs::write(dir.join("a.txt"), "").unwrap();
    fs::write(dir.join("b.txt"), "").unwrap();
    let input = JsonValue::parse(&format!(r#"{{"path":"{}"}}"#, dir.display())).unwrap();
    let result = LsTool.execute(&input).unwrap();
    assert!(result.contains("a.txt"));
    assert!(result.contains("b.txt"));
}

#[test]
fn ls_directories_have_trailing_slash() {
    let dir = tempdir();
    fs::create_dir(dir.join("subdir")).unwrap();
    let input = JsonValue::parse(&format!(r#"{{"path":"{}"}}"#, dir.display())).unwrap();
    let result = LsTool.execute(&input).unwrap();
    assert!(result.contains("subdir/"));
}

#[test]
fn ls_no_path_uses_current_dir() {
    let result = LsTool.execute(&JsonValue::Object(vec![])).unwrap();
    assert!(!result.is_empty());
}
```

Create `tests/tools/file/glob_test.rs`:
```rust
use std::fs;
use viv::json::JsonValue;
use viv::tools::Tool;
use viv::tools::file::glob::GlobTool;

fn tempdir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("viv_glob_{}", nanos()));
    fs::create_dir_all(&p).unwrap();
    p
}
fn nanos() -> u32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos()
}

#[test]
fn glob_star_matches_extension() {
    let dir = tempdir();
    fs::write(dir.join("a.rs"), "").unwrap();
    fs::write(dir.join("b.rs"), "").unwrap();
    fs::write(dir.join("c.txt"), "").unwrap();
    let input = JsonValue::parse(&format!(r#"{{"pattern":"*.rs","path":"{}"}}"#, dir.display())).unwrap();
    let result = GlobTool.execute(&input).unwrap();
    assert!(result.contains("a.rs"));
    assert!(result.contains("b.rs"));
    assert!(!result.contains("c.txt"));
}

#[test]
fn glob_double_star_recurses() {
    let dir = tempdir();
    let sub = dir.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("deep.rs"), "").unwrap();
    fs::write(dir.join("top.rs"), "").unwrap();
    let input = JsonValue::parse(&format!(r#"{{"pattern":"**/*.rs","path":"{}"}}"#, dir.display())).unwrap();
    let result = GlobTool.execute(&input).unwrap();
    assert!(result.contains("deep.rs"));
    assert!(result.contains("top.rs"));
}
```

- [ ] **Step 2: Run tests — verify they fail**

```bash
cargo test --test read_test --test ls_test --test glob_test 2>&1 | head -20
```
Expected: FAIL.

- [ ] **Step 3: Implement `src/tools/file/read.rs`**

```rust
use crate::error::Error;
use crate::json::JsonValue;
use crate::tools::{PermissionLevel, Tool};

pub struct ReadTool;

impl Tool for ReadTool {
    fn name(&self) -> &str { "read" }

    fn description(&self) -> &str {
        "Read a file and return its contents with line numbers. Use offset (1-based line) and limit to read a specific range."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "file_path":{"type":"string","description":"Path to the file"},
                "offset":{"type":"number","description":"Line to start reading from (1-based, default 1)"},
                "limit":{"type":"number","description":"Maximum number of lines to return"}
            },
            "required":["file_path"]
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> crate::Result<String> {
        let path = input.get("file_path").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'file_path'".into()))?;

        let content = std::fs::read_to_string(path)
            .map_err(|e| Error::Tool(format!("cannot read '{}': {}", path, e)))?;

        let offset = input.get("offset").and_then(|v| v.as_i64()).unwrap_or(1).max(1) as usize;
        let limit = input.get("limit").and_then(|v| v.as_i64()).map(|n| n as usize);

        let lines: Vec<&str> = content.lines().collect();
        let start = (offset - 1).min(lines.len());
        let end = limit.map(|l| (start + l).min(lines.len())).unwrap_or(lines.len());

        let mut out = String::new();
        for (i, line) in lines[start..end].iter().enumerate() {
            out.push_str(&format!("{}\t{}\n", start + i + 1, line));
        }
        Ok(out)
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }
}
```

- [ ] **Step 4: Implement `src/tools/file/ls.rs`**

```rust
use crate::error::Error;
use crate::json::JsonValue;
use crate::tools::{PermissionLevel, Tool};

pub struct LsTool;

impl Tool for LsTool {
    fn name(&self) -> &str { "ls" }

    fn description(&self) -> &str {
        "List directory contents. Directories are shown with a trailing slash. Entries are sorted alphabetically."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "path":{"type":"string","description":"Directory to list (default: current directory)"}
            }
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> crate::Result<String> {
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let mut entries: Vec<String> = std::fs::read_dir(path)
            .map_err(|e| Error::Tool(format!("cannot list '{}': {}", path, e)))?
            .filter_map(|e| e.ok())
            .map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    format!("{}/", name)
                } else {
                    name
                }
            })
            .collect();

        entries.sort();
        Ok(entries.join("\n"))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }
}
```

- [ ] **Step 5: Implement `src/tools/file/glob.rs`**

```rust
use crate::error::Error;
use crate::json::JsonValue;
use crate::tools::{PermissionLevel, Tool};
use std::path::{Path, PathBuf};

pub struct GlobTool;

impl Tool for GlobTool {
    fn name(&self) -> &str { "glob" }

    fn description(&self) -> &str {
        "Find files matching a glob pattern. Supports * (any chars in one segment), ** (any path depth), ? (one char)."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "pattern":{"type":"string","description":"Glob pattern, e.g. \"**/*.rs\""},
                "path":{"type":"string","description":"Root directory to search (default: current directory)"}
            },
            "required":["pattern"]
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> crate::Result<String> {
        let pattern = input.get("pattern").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'pattern'".into()))?;
        let root = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let mut matches: Vec<PathBuf> = vec![];
        let parts: Vec<&str> = pattern.split('/').collect();
        walk_glob(Path::new(root), &parts, &mut matches)
            .map_err(|e| Error::Tool(format!("glob walk: {}", e)))?;
        matches.sort();
        Ok(matches.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n"))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }
}

fn walk_glob(dir: &Path, parts: &[&str], out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if parts.is_empty() {
        return Ok(());
    }
    let seg = parts[0];
    let rest = &parts[1..];

    if seg == "**" {
        // Match zero segments: try continuing with rest from current dir
        if !rest.is_empty() {
            walk_glob(dir, rest, out)?;
        } else {
            collect_all(dir, out)?;
            return Ok(());
        }
        // Match one or more: recurse into subdirs keeping ** active
        if dir.is_dir() {
            for entry in std::fs::read_dir(dir)?.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    walk_glob(&p, parts, out)?;
                }
            }
        }
        return Ok(());
    }

    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)?.flatten() {
            let p = entry.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if segment_match(seg, name) {
                if rest.is_empty() {
                    out.push(p);
                } else if p.is_dir() {
                    walk_glob(&p, rest, out)?;
                }
            }
        }
    }
    Ok(())
}

fn collect_all(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)?.flatten() {
            let p = entry.path();
            out.push(p.clone());
            if p.is_dir() {
                collect_all(&p, out)?;
            }
        }
    }
    Ok(())
}

/// DP wildcard match: `*` = any chars, `?` = one char (within one path segment)
fn segment_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (m, n) = (p.len(), t.len());
    let mut dp = vec![vec![false; n + 1]; m + 1];
    dp[0][0] = true;
    for i in 1..=m {
        if p[i - 1] == '*' { dp[i][0] = dp[i - 1][0]; }
    }
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if p[i - 1] == '*' {
                dp[i - 1][j] || dp[i][j - 1]
            } else if p[i - 1] == '?' || p[i - 1] == t[j - 1] {
                dp[i - 1][j - 1]
            } else {
                false
            };
        }
    }
    dp[m][n]
}
```

- [ ] **Step 6: Run tests — verify they pass**

```bash
cargo test --test read_test --test ls_test --test glob_test 2>&1
```
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/tools/file/read.rs src/tools/file/ls.rs src/tools/file/glob.rs \
        tests/tools/file/
git commit -m "feat(tools): Read + LS + Glob (ReadOnly file tools)"
```

---

### Task 4: Write file tools — Write, Edit, MultiEdit, Grep

**Files:**
- Modify: `src/tools/file/write.rs`
- Modify: `src/tools/file/edit.rs`
- Modify: `src/tools/file/grep.rs`
- Create: `tests/tools/file/write_test.rs`
- Create: `tests/tools/file/edit_test.rs`
- Create: `tests/tools/file/grep_test.rs`

- [ ] **Step 1: Write the failing tests**

Create `tests/tools/file/write_test.rs`:
```rust
use std::fs;
use viv::json::JsonValue;
use viv::tools::Tool;
use viv::tools::file::write::WriteTool;

fn tempdir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("viv_write_{}", nanos()));
    fs::create_dir_all(&p).unwrap();
    p
}
fn nanos() -> u32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos()
}

#[test]
fn write_creates_file_with_content() {
    let dir = tempdir();
    let path = dir.join("out.txt");
    let input = JsonValue::parse(&format!(
        r#"{{"file_path":"{}","content":"hello world"}}"#, path.display()
    )).unwrap();
    WriteTool.execute(&input).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "hello world");
}

#[test]
fn write_creates_parent_directories() {
    let dir = tempdir();
    let path = dir.join("a/b/c.txt");
    let input = JsonValue::parse(&format!(
        r#"{{"file_path":"{}","content":"hi"}}"#, path.display()
    )).unwrap();
    WriteTool.execute(&input).unwrap();
    assert!(path.exists());
}

#[test]
fn write_overwrites_existing_file() {
    let dir = tempdir();
    let path = dir.join("f.txt");
    fs::write(&path, "old").unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"file_path":"{}","content":"new"}}"#, path.display()
    )).unwrap();
    WriteTool.execute(&input).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "new");
}
```

Create `tests/tools/file/edit_test.rs`:
```rust
use std::fs;
use viv::json::JsonValue;
use viv::tools::Tool;
use viv::tools::file::edit::{EditTool, MultiEditTool};

fn tempdir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("viv_edit_{}", nanos()));
    fs::create_dir_all(&p).unwrap();
    p
}
fn nanos() -> u32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos()
}

#[test]
fn edit_replaces_unique_string() {
    let dir = tempdir();
    let path = dir.join("f.txt");
    fs::write(&path, "hello world").unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"file_path":"{}","old_string":"world","new_string":"rust"}}"#, path.display()
    )).unwrap();
    EditTool.execute(&input).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "hello rust");
}

#[test]
fn edit_fails_when_old_string_not_found() {
    let dir = tempdir();
    let path = dir.join("f.txt");
    fs::write(&path, "hello world").unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"file_path":"{}","old_string":"missing","new_string":"x"}}"#, path.display()
    )).unwrap();
    assert!(EditTool.execute(&input).is_err());
}

#[test]
fn edit_fails_when_old_string_not_unique() {
    let dir = tempdir();
    let path = dir.join("f.txt");
    fs::write(&path, "a a a").unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"file_path":"{}","old_string":"a","new_string":"b"}}"#, path.display()
    )).unwrap();
    assert!(EditTool.execute(&input).is_err());
}

#[test]
fn edit_replace_all_replaces_every_occurrence() {
    let dir = tempdir();
    let path = dir.join("f.txt");
    fs::write(&path, "a a a").unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"file_path":"{}","old_string":"a","new_string":"b","replace_all":true}}"#, path.display()
    )).unwrap();
    EditTool.execute(&input).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "b b b");
}

#[test]
fn multi_edit_applies_edits_in_order() {
    let dir = tempdir();
    let path = dir.join("f.txt");
    fs::write(&path, "hello world foo").unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"file_path":"{}","edits":[{{"old_string":"hello","new_string":"hi"}},{{"old_string":"foo","new_string":"bar"}}]}}"#,
        path.display()
    )).unwrap();
    MultiEditTool.execute(&input).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "hi world bar");
}
```

Create `tests/tools/file/grep_test.rs`:
```rust
use std::fs;
use viv::json::JsonValue;
use viv::tools::Tool;
use viv::tools::file::grep::GrepTool;

fn tempdir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("viv_grep_{}", nanos()));
    fs::create_dir_all(&p).unwrap();
    p
}
fn nanos() -> u32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos()
}

#[test]
fn grep_files_with_matches_default_mode() {
    let dir = tempdir();
    fs::write(dir.join("a.txt"), "hello world").unwrap();
    fs::write(dir.join("b.txt"), "goodbye world").unwrap();
    fs::write(dir.join("c.txt"), "nothing here").unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"pattern":"world","path":"{}"}}"#, dir.display()
    )).unwrap();
    let result = GrepTool.execute(&input).unwrap();
    assert!(result.contains("a.txt"));
    assert!(result.contains("b.txt"));
    assert!(!result.contains("c.txt"));
}

#[test]
fn grep_content_mode_shows_matching_lines() {
    let dir = tempdir();
    fs::write(dir.join("a.txt"), "line1\nfoo bar\nline3").unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"pattern":"foo","path":"{}","output_mode":"content"}}"#, dir.display()
    )).unwrap();
    let result = GrepTool.execute(&input).unwrap();
    assert!(result.contains("foo bar"));
    assert!(!result.contains("line1"));
}

#[test]
fn grep_glob_filter_limits_files() {
    let dir = tempdir();
    fs::write(dir.join("a.rs"), "fn main() {}").unwrap();
    fs::write(dir.join("b.txt"), "fn main() {}").unwrap();
    let input = JsonValue::parse(&format!(
        r#"{{"pattern":"main","path":"{}","glob":"*.rs"}}"#, dir.display()
    )).unwrap();
    let result = GrepTool.execute(&input).unwrap();
    assert!(result.contains("a.rs"));
    assert!(!result.contains("b.txt"));
}
```

- [ ] **Step 2: Run tests — verify they fail**

```bash
cargo test --test write_test --test edit_test --test grep_test 2>&1 | head -20
```
Expected: FAIL.

- [ ] **Step 3: Implement `src/tools/file/write.rs`**

```rust
use crate::error::Error;
use crate::json::JsonValue;
use crate::tools::{PermissionLevel, Tool};

pub struct WriteTool;

impl Tool for WriteTool {
    fn name(&self) -> &str { "write" }

    fn description(&self) -> &str {
        "Write content to a file, overwriting it if it already exists. Creates parent directories automatically."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "file_path":{"type":"string","description":"Path to the file to write"},
                "content":{"type":"string","description":"Content to write"}
            },
            "required":["file_path","content"]
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> crate::Result<String> {
        let path = input.get("file_path").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'file_path'".into()))?;
        let content = input.get("content").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'content'".into()))?;

        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::Tool(e.to_string()))?;
        }
        std::fs::write(path, content)
            .map_err(|e| Error::Tool(format!("write '{}': {}", path, e)))?;
        Ok(format!("Wrote {} bytes to {}", content.len(), path))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::Write }
}
```

- [ ] **Step 4: Implement `src/tools/file/edit.rs`**

```rust
use crate::error::Error;
use crate::json::JsonValue;
use crate::tools::{PermissionLevel, Tool};

pub struct EditTool;

impl Tool for EditTool {
    fn name(&self) -> &str { "edit" }

    fn description(&self) -> &str {
        "Replace an exact string in a file. Fails if old_string is not found or appears more than once (unless replace_all: true)."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "file_path":{"type":"string"},
                "old_string":{"type":"string","description":"Exact text to replace"},
                "new_string":{"type":"string","description":"Replacement text"},
                "replace_all":{"type":"boolean","description":"Replace all occurrences (default: false)"}
            },
            "required":["file_path","old_string","new_string"]
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> crate::Result<String> {
        let path = input.get("file_path").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'file_path'".into()))?;
        let old = input.get("old_string").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'old_string'".into()))?;
        let new = input.get("new_string").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'new_string'".into()))?;
        let replace_all = input.get("replace_all").and_then(|v| v.as_bool()).unwrap_or(false);

        let content = std::fs::read_to_string(path)
            .map_err(|e| Error::Tool(format!("read '{}': {}", path, e)))?;

        let count = content.matches(old).count();
        if count == 0 {
            return Err(Error::Tool(format!("'old_string' not found in '{}'", path)));
        }
        if count > 1 && !replace_all {
            return Err(Error::Tool(format!(
                "'old_string' appears {} times in '{}'; use replace_all: true", count, path
            )));
        }

        let new_content = if replace_all { content.replace(old, new) } else { content.replacen(old, new, 1) };
        std::fs::write(path, &new_content)
            .map_err(|e| Error::Tool(format!("write '{}': {}", path, e)))?;
        Ok(format!("Replaced {} occurrence(s) in {}", if replace_all { count } else { 1 }, path))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::Write }
}

pub struct MultiEditTool;

impl Tool for MultiEditTool {
    fn name(&self) -> &str { "multi_edit" }

    fn description(&self) -> &str {
        "Apply multiple edits to one file in order. Each edit is {old_string, new_string, replace_all?}. All edits succeed or none are written."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "file_path":{"type":"string"},
                "edits":{
                    "type":"array",
                    "items":{
                        "type":"object",
                        "properties":{
                            "old_string":{"type":"string"},
                            "new_string":{"type":"string"},
                            "replace_all":{"type":"boolean"}
                        },
                        "required":["old_string","new_string"]
                    }
                }
            },
            "required":["file_path","edits"]
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> crate::Result<String> {
        let path = input.get("file_path").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'file_path'".into()))?;
        let edits = input.get("edits").and_then(|v| v.as_array())
            .ok_or_else(|| Error::Tool("missing 'edits'".into()))?;

        let mut content = std::fs::read_to_string(path)
            .map_err(|e| Error::Tool(format!("read '{}': {}", path, e)))?;

        for (i, edit) in edits.iter().enumerate() {
            let old = edit.get("old_string").and_then(|v| v.as_str())
                .ok_or_else(|| Error::Tool(format!("edits[{}] missing 'old_string'", i)))?;
            let new = edit.get("new_string").and_then(|v| v.as_str())
                .ok_or_else(|| Error::Tool(format!("edits[{}] missing 'new_string'", i)))?;
            let replace_all = edit.get("replace_all").and_then(|v| v.as_bool()).unwrap_or(false);

            let count = content.matches(old).count();
            if count == 0 {
                return Err(Error::Tool(format!("edits[{}]: 'old_string' not found", i)));
            }
            if count > 1 && !replace_all {
                return Err(Error::Tool(format!(
                    "edits[{}]: 'old_string' appears {} times; use replace_all: true", i, count
                )));
            }
            content = if replace_all { content.replace(old, new) } else { content.replacen(old, new, 1) };
        }

        std::fs::write(path, &content)
            .map_err(|e| Error::Tool(format!("write '{}': {}", path, e)))?;
        Ok(format!("Applied {} edit(s) to {}", edits.len(), path))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::Write }
}
```

- [ ] **Step 5: Implement `src/tools/file/grep.rs`**

```rust
use crate::error::Error;
use crate::json::JsonValue;
use crate::tools::{PermissionLevel, Tool};
use std::path::{Path, PathBuf};

pub struct GrepTool;

impl Tool for GrepTool {
    fn name(&self) -> &str { "grep" }

    fn description(&self) -> &str {
        "Search for a literal pattern in files. output_mode: 'files_with_matches' (default), 'content' (show matching lines with line numbers), 'count'."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "pattern":{"type":"string","description":"Literal string to search for"},
                "path":{"type":"string","description":"File or directory to search (default: current directory)"},
                "glob":{"type":"string","description":"Filename glob filter, e.g. \"*.rs\""},
                "output_mode":{"type":"string","description":"files_with_matches | content | count"}
            },
            "required":["pattern"]
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> crate::Result<String> {
        let pattern = input.get("pattern").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'pattern'".into()))?;
        let root = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let glob_filter = input.get("glob").and_then(|v| v.as_str());
        let mode = input.get("output_mode").and_then(|v| v.as_str()).unwrap_or("files_with_matches");

        let mut files: Vec<PathBuf> = vec![];
        collect_files(Path::new(root), glob_filter, &mut files)
            .map_err(|e| Error::Tool(format!("walk: {}", e)))?;
        files.sort();

        let mut out = String::new();
        for file in &files {
            let content = match std::fs::read_to_string(file) { Ok(c) => c, Err(_) => continue };
            match mode {
                "content" => {
                    for (i, line) in content.lines().enumerate() {
                        if line.contains(pattern) {
                            out.push_str(&format!("{}:{}:{}\n", file.display(), i + 1, line));
                        }
                    }
                }
                "count" => {
                    let c = content.lines().filter(|l| l.contains(pattern)).count();
                    if c > 0 { out.push_str(&format!("{}:{}\n", file.display(), c)); }
                }
                _ => {
                    if content.contains(pattern) { out.push_str(&format!("{}\n", file.display())); }
                }
            }
        }
        Ok(out.trim_end().to_string())
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }
}

fn collect_files(dir: &Path, filter: Option<&str>, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if dir.is_file() {
        out.push(dir.to_path_buf());
        return Ok(());
    }
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)?.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_files(&p, filter, out)?;
            } else {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if filter.map(|f| segment_match(f, name)).unwrap_or(true) {
                    out.push(p);
                }
            }
        }
    }
    Ok(())
}

fn segment_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (m, n) = (p.len(), t.len());
    let mut dp = vec![vec![false; n + 1]; m + 1];
    dp[0][0] = true;
    for i in 1..=m { if p[i-1] == '*' { dp[i][0] = dp[i-1][0]; } }
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if p[i-1] == '*' { dp[i-1][j] || dp[i][j-1] }
                       else if p[i-1] == '?' || p[i-1] == t[j-1] { dp[i-1][j-1] }
                       else { false };
        }
    }
    dp[m][n]
}
```

- [ ] **Step 6: Run tests — verify they pass**

```bash
cargo test --test write_test --test edit_test --test grep_test 2>&1
```
Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/tools/file/write.rs src/tools/file/edit.rs src/tools/file/grep.rs \
        tests/tools/file/write_test.rs tests/tools/file/edit_test.rs tests/tools/file/grep_test.rs
git commit -m "feat(tools): Write + Edit + MultiEdit + Grep file tools"
```

---

### Task 5: Bash + BashBackground

**Files:**
- Modify: `src/tools/bash.rs`
- Create: `tests/tools/bash_test.rs`

- [ ] **Step 1: Write the failing tests**

Create `tests/tools/bash_test.rs`:
```rust
use viv::json::JsonValue;
use viv::tools::Tool;
use viv::tools::bash::{BashBackgroundTool, BashTool};

#[test]
fn bash_captures_stdout() {
    let input = JsonValue::parse(r#"{"command":"echo hello"}"#).unwrap();
    let result = BashTool.execute(&input).unwrap();
    assert!(result.contains("hello"));
}

#[test]
fn bash_captures_stderr() {
    let input = JsonValue::parse(r#"{"command":"echo error >&2"}"#).unwrap();
    let result = BashTool.execute(&input).unwrap();
    assert!(result.contains("error"));
}

#[test]
fn bash_nonzero_exit_code_in_output() {
    let input = JsonValue::parse(r#"{"command":"exit 42"}"#).unwrap();
    let result = BashTool.execute(&input).unwrap();
    assert!(result.contains("42"));
}

#[test]
fn bash_timeout_returns_error() {
    let input = JsonValue::parse(r#"{"command":"sleep 10","timeout_ms":100}"#).unwrap();
    assert!(BashTool.execute(&input).is_err());
}

#[test]
fn bash_background_returns_pid_line() {
    let input = JsonValue::parse(r#"{"command":"sleep 1","description":"test sleep"}"#).unwrap();
    let result = BashBackgroundTool.execute(&input).unwrap();
    assert!(result.contains("pid"));
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test --test bash_test 2>&1 | head -15
```
Expected: FAIL.

- [ ] **Step 3: Implement `src/tools/bash.rs`**

```rust
use crate::error::Error;
use crate::json::JsonValue;
use crate::tools::{PermissionLevel, Tool};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct BashTool;

impl Tool for BashTool {
    fn name(&self) -> &str { "bash" }

    fn description(&self) -> &str {
        "Execute a shell command and return stdout + stderr. Fails with an error if the command times out."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "command":{"type":"string","description":"Shell command to run"},
                "timeout_ms":{"type":"number","description":"Timeout in ms (default: 30000)"}
            },
            "required":["command"]
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> crate::Result<String> {
        let command = input.get("command").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'command'".into()))?;
        let timeout_ms = input.get("timeout_ms").and_then(|v| v.as_i64()).unwrap_or(30_000) as u64;

        let mut child = Command::new("sh")
            .arg("-c").arg(command)
            .stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::Tool(format!("spawn: {}", e)))?;

        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            if child.try_wait().map_err(|e| Error::Tool(e.to_string()))?.is_some() { break; }
            if Instant::now() >= deadline {
                let _ = child.kill();
                return Err(Error::Tool(format!("timed out after {}ms", timeout_ms)));
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        let output = child.wait_with_output().map_err(|e| Error::Tool(e.to_string()))?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let code = output.status.code().unwrap_or(-1);

        let mut result = stdout;
        if !stderr.is_empty() {
            if !result.is_empty() { result.push('\n'); }
            result.push_str(&stderr);
        }
        if code != 0 {
            if !result.is_empty() { result.push('\n'); }
            result.push_str(&format!("Exit code: {}", code));
        }
        Ok(result)
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::Execute }
}

pub struct BashBackgroundTool;

impl Tool for BashBackgroundTool {
    fn name(&self) -> &str { "bash_background" }

    fn description(&self) -> &str {
        "Start a shell command in the background and return its process ID. The process runs independently of the agent."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "command":{"type":"string","description":"Shell command to run in background"},
                "description":{"type":"string","description":"What this process does"}
            },
            "required":["command","description"]
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> crate::Result<String> {
        let command = input.get("command").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'command'".into()))?;
        let description = input.get("description").and_then(|v| v.as_str()).unwrap_or("");

        let child = Command::new("sh")
            .arg("-c").arg(command)
            .stdout(Stdio::null()).stderr(Stdio::null())
            .spawn()
            .map_err(|e| Error::Tool(format!("spawn: {}", e)))?;

        Ok(format!("Started background process (pid: {}): {}", child.id(), description))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::Execute }
}
```

- [ ] **Step 4: Run tests — verify they pass**

```bash
cargo test --test bash_test 2>&1
```
Expected: `test result: ok. 5 passed`

- [ ] **Step 5: Commit**

```bash
git add src/tools/bash.rs tests/tools/bash_test.rs
git commit -m "feat(tools): Bash + BashBackground tools"
```

---

### Task 6: TodoWrite + TodoRead + WebFetch

**Files:**
- Modify: `src/tools/todo.rs`
- Modify: `src/tools/web.rs`
- Create: `tests/tools/todo_test.rs`

- [ ] **Step 1: Write the failing tests**

Create `tests/tools/todo_test.rs`:
```rust
use std::fs;
use viv::json::JsonValue;
use viv::tools::Tool;
use viv::tools::todo::{TodoReadTool, TodoWriteTool};

fn tempdir() -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("viv_todo_{}", nanos()));
    fs::create_dir_all(&p).unwrap();
    p
}
fn nanos() -> u32 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos()
}

#[test]
fn todo_write_then_read_roundtrip() {
    let dir = tempdir();
    let path = dir.join("todo.json");
    let write = TodoWriteTool::new(path.clone());
    let input = JsonValue::parse(
        r#"{"todos":[{"id":"1","content":"buy milk","status":"pending"}]}"#
    ).unwrap();
    write.execute(&input).unwrap();

    let read = TodoReadTool::new(path);
    let result = read.execute(&JsonValue::Object(vec![])).unwrap();
    assert!(result.contains("buy milk"));
    assert!(result.contains("pending"));
}

#[test]
fn todo_read_returns_empty_array_when_no_file() {
    let path = std::path::PathBuf::from("/nonexistent/viv_todo.json");
    let read = TodoReadTool::new(path);
    let result = read.execute(&JsonValue::Object(vec![])).unwrap();
    assert_eq!(result, "[]");
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test --test todo_test 2>&1 | head -10
```
Expected: FAIL.

- [ ] **Step 3: Implement `src/tools/todo.rs`**

```rust
use std::path::PathBuf;
use crate::error::Error;
use crate::json::JsonValue;
use crate::tools::{PermissionLevel, Tool};

pub struct TodoWriteTool { path: PathBuf }
impl TodoWriteTool {
    pub fn new(path: PathBuf) -> Self { TodoWriteTool { path } }
}

impl Tool for TodoWriteTool {
    fn name(&self) -> &str { "todo_write" }

    fn description(&self) -> &str {
        "Write the task list. Replaces all existing todos. Each todo needs id, content, and status (pending | in_progress | completed)."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "todos":{
                    "type":"array",
                    "items":{
                        "type":"object",
                        "properties":{
                            "id":{"type":"string"},
                            "content":{"type":"string"},
                            "status":{"type":"string"}
                        },
                        "required":["id","content","status"]
                    }
                }
            },
            "required":["todos"]
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> crate::Result<String> {
        let todos = input.get("todos").ok_or_else(|| Error::Tool("missing 'todos'".into()))?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::Tool(e.to_string()))?;
        }
        std::fs::write(&self.path, format!("{}", todos))
            .map_err(|e| Error::Tool(e.to_string()))?;
        let count = todos.as_array().map(|a| a.len()).unwrap_or(0);
        Ok(format!("Wrote {} todo(s)", count))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::Write }
}

pub struct TodoReadTool { path: PathBuf }
impl TodoReadTool {
    pub fn new(path: PathBuf) -> Self { TodoReadTool { path } }
}

impl Tool for TodoReadTool {
    fn name(&self) -> &str { "todo_read" }
    fn description(&self) -> &str { "Read the current task list. Returns a JSON array of todos." }
    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{"type":"object","properties":{}}"#).unwrap()
    }

    fn execute(&self, _input: &JsonValue) -> crate::Result<String> {
        if !self.path.exists() { return Ok("[]".into()); }
        std::fs::read_to_string(&self.path).map_err(|e| Error::Tool(e.to_string()))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }
}
```

- [ ] **Step 4: Implement `src/tools/web.rs`**

```rust
use std::io::{Read, Write};
use std::sync::Arc;
use crate::error::Error;
use crate::json::JsonValue;
use crate::llm::{LLMClient, ModelTier};
use crate::net::http::HttpRequest;
use crate::net::tls::TlsStream;
use crate::tools::{PermissionLevel, Tool};

pub struct WebFetchTool { llm: Arc<LLMClient> }
impl WebFetchTool {
    pub fn new(llm: Arc<LLMClient>) -> Self { WebFetchTool { llm } }
}

impl Tool for WebFetchTool {
    fn name(&self) -> &str { "web_fetch" }

    fn description(&self) -> &str {
        "Fetch a URL over HTTPS and return its text content. If prompt is given, an LLM extracts the relevant part."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::parse(r#"{
            "type":"object",
            "properties":{
                "url":{"type":"string","description":"HTTPS URL to fetch"},
                "prompt":{"type":"string","description":"What to extract from the page"}
            },
            "required":["url"]
        }"#).unwrap()
    }

    fn execute(&self, input: &JsonValue) -> crate::Result<String> {
        let url = input.get("url").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Tool("missing 'url'".into()))?;
        let prompt = input.get("prompt").and_then(|v| v.as_str()).unwrap_or("");

        let text = fetch_url(url)?;
        let truncated: String = text.chars().take(8000).collect();

        if prompt.is_empty() {
            return Ok(truncated);
        }

        use crate::agent::message::{Message, SystemBlock};
        let system = vec![SystemBlock::dynamic("You extract relevant content from web pages.")];
        let user_msg = format!("Answer this about the page: {}\n\nPage:\n{}", prompt, truncated);
        let messages = vec![Message::user_text(user_msg)];
        let mut response = String::new();
        self.llm.stream_agent(&system, &messages, "", ModelTier::Fast, |t| response.push_str(t))?;
        Ok(response)
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::Execute }
}

fn fetch_url(url: &str) -> crate::Result<String> {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://")).unwrap_or(url);
    let (host_port, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match host_port.rfind(':') {
        Some(i) => (
            &host_port[..i],
            host_port[i + 1..].parse::<u16>().unwrap_or(443),
        ),
        None => (host_port, 443),
    };

    let req = HttpRequest {
        method: "GET".into(),
        path: path.to_string(),
        headers: vec![
            ("Host".into(), host.to_string()),
            ("User-Agent".into(), "viv/0.1".into()),
            ("Accept".into(), "text/html,text/plain".into()),
            ("Connection".into(), "close".into()),
        ],
        body: None,
    };

    let mut tls = TlsStream::connect(host, port)?;
    tls.write_all(&req.to_bytes())?;

    let mut raw: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        let n = tls.read(&mut tmp)?;
        if n == 0 || raw.len() > 1_000_000 { break; }
        raw.extend_from_slice(&tmp[..n]);
    }

    let body = raw.windows(4).position(|w| w == b"\r\n\r\n")
        .map(|i| &raw[i + 4..])
        .unwrap_or(&raw);

    Ok(strip_html(&String::from_utf8_lossy(body)))
}

fn strip_html(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    let mut last_space = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if in_tag => {}
            ' ' | '\t' => { if !last_space { out.push(' '); last_space = true; } }
            '\n' | '\r' => { out.push('\n'); last_space = false; }
            _ => { out.push(ch); last_space = false; }
        }
    }
    out
}
```

- [ ] **Step 5: Run todo tests — verify they pass**

```bash
cargo test --test todo_test 2>&1
```
Expected: `test result: ok. 2 passed`

- [ ] **Step 6: Build clean**

```bash
cargo build 2>&1 | grep "^error"
```
Expected: no errors.

- [ ] **Step 7: Commit**

```bash
git add src/tools/todo.rs src/tools/web.rs tests/tools/todo_test.rs
git commit -m "feat(tools): TodoWrite + TodoRead + WebFetch tools"
```

---

### Task 7: LLM stream_agent — add tools_json parameter

**Files:**
- Modify: `src/llm.rs` (lines ~321–404: stream_agent + build_agent_request)
- Modify: `src/memory/retrieval.rs:67`
- Modify: `src/memory/compaction.rs:31`
- Modify: `src/agent/evolution.rs:41` and `:110`

- [ ] **Step 1: Update `stream_agent` signature in `src/llm.rs`**

The current signature (around line 321):
```rust
pub fn stream_agent(
    &self,
    system_blocks: &[crate::agent::message::SystemBlock],
    messages: &[crate::agent::message::Message],
    tier: ModelTier,
    mut on_text: impl FnMut(&str),
) -> crate::Result<StreamResult>
```

Replace with (add `tools_json: &str` as third argument, pass to `build_agent_request`):
```rust
pub fn stream_agent(
    &self,
    system_blocks: &[crate::agent::message::SystemBlock],
    messages: &[crate::agent::message::Message],
    tools_json: &str,
    tier: ModelTier,
    mut on_text: impl FnMut(&str),
) -> crate::Result<StreamResult> {
    let req = self.build_agent_request(system_blocks, messages, tools_json, tier);
    // ... rest of body unchanged
```

- [ ] **Step 2: Update `build_agent_request` in `src/llm.rs`**

Current signature (around line 371):
```rust
fn build_agent_request(
    &self,
    system_blocks: &[crate::agent::message::SystemBlock],
    messages: &[crate::agent::message::Message],
    tier: ModelTier,
) -> HttpRequest
```

Replace with:
```rust
fn build_agent_request(
    &self,
    system_blocks: &[crate::agent::message::SystemBlock],
    messages: &[crate::agent::message::Message],
    tools_json: &str,
    tier: ModelTier,
) -> HttpRequest {
    let model = self.config.model(tier.clone()).to_string();
    let max_tokens = self.config.max_tokens(tier);
    let url = parse_base_url(&self.config.base_url);

    let system_json: Vec<String> = system_blocks.iter().map(|b| b.to_json()).collect();
    let messages_json: Vec<String> = messages.iter().map(|m| m.to_json()).collect();

    let body = if tools_json.is_empty() {
        format!(
            "{{\"model\":{},\"max_tokens\":{},\"stream\":true,\"system\":[{}],\"messages\":[{}]}}",
            JsonValue::Str(model), max_tokens,
            system_json.join(","), messages_json.join(","),
        )
    } else {
        format!(
            "{{\"model\":{},\"max_tokens\":{},\"stream\":true,\"tools\":{},\"system\":[{}],\"messages\":[{}]}}",
            JsonValue::Str(model), max_tokens, tools_json,
            system_json.join(","), messages_json.join(","),
        )
    };

    HttpRequest {
        method: "POST".into(),
        path: format!("{}/v1/messages", url.path_prefix),
        headers: vec![
            ("Host".into(), url.host),
            ("Content-Type".into(), "application/json".into()),
            ("x-api-key".into(), self.config.api_key.clone()),
            ("anthropic-version".into(), "2023-06-01".into()),
            ("anthropic-beta".into(), "prompt-caching-2024-07-31".into()),
        ],
        body: Some(body),
    }
}
```

- [ ] **Step 3: Fix all callers — add `""` for tools_json**

In `src/memory/retrieval.rs` at line 67, change:
```rust
llm.stream_agent(&system, &messages, ModelTier::Fast, |t| response.push_str(t))?;
```
to:
```rust
llm.stream_agent(&system, &messages, "", ModelTier::Fast, |t| response.push_str(t))?;
```

In `src/memory/compaction.rs` at line 31, change:
```rust
llm.stream_agent(&system, &req_msgs, ModelTier::Fast, |t| summary.push_str(t))?;
```
to:
```rust
llm.stream_agent(&system, &req_msgs, "", ModelTier::Fast, |t| summary.push_str(t))?;
```

In `src/agent/evolution.rs` at line 41, change:
```rust
llm.stream_agent(&system, &req_msgs, ModelTier::Medium, |t| response.push_str(t))?;
```
to:
```rust
llm.stream_agent(&system, &req_msgs, "", ModelTier::Medium, |t| response.push_str(t))?;
```

In `src/agent/evolution.rs` at line 110, change:
```rust
llm.stream_agent(&system, &req_msgs, ModelTier::Fast, |t| summary.push_str(t))?;
```
to:
```rust
llm.stream_agent(&system, &req_msgs, "", ModelTier::Fast, |t| summary.push_str(t))?;
```

- [ ] **Step 4: Build and verify clean**

```bash
cargo build 2>&1 | grep "^error"
```
Expected: no errors.

- [ ] **Step 5: Run all tests**

```bash
cargo test 2>&1 | tail -5
```
Expected: all existing tests still pass.

- [ ] **Step 6: Commit**

```bash
git add src/llm.rs src/memory/retrieval.rs src/memory/compaction.rs src/agent/evolution.rs
git commit -m "feat(llm): add tools_json param to stream_agent for Anthropic tools array"
```

---

### Task 8: Populate ToolRegistry + AgentContext + run_agent real dispatch

**Files:**
- Modify: `src/tools/mod.rs` (populate `default_tools`)
- Modify: `src/agent/context.rs`
- Modify: `src/agent/run.rs`

- [ ] **Step 1: Populate `default_tools` in `src/tools/mod.rs`**

Replace the body of `default_tools`:
```rust
pub fn default_tools(llm: std::sync::Arc<crate::llm::LLMClient>) -> Self {
    use crate::tools::bash::{BashBackgroundTool, BashTool};
    use crate::tools::file::edit::{EditTool, MultiEditTool};
    use crate::tools::file::glob::GlobTool;
    use crate::tools::file::grep::GrepTool;
    use crate::tools::file::ls::LsTool;
    use crate::tools::file::read::ReadTool;
    use crate::tools::file::write::WriteTool;
    use crate::tools::todo::{TodoReadTool, TodoWriteTool};
    use crate::tools::web::WebFetchTool;

    let todo_path = std::path::PathBuf::from(".viv/todo.json");
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(BashTool));
    reg.register(Box::new(BashBackgroundTool));
    reg.register(Box::new(ReadTool));
    reg.register(Box::new(WriteTool));
    reg.register(Box::new(EditTool));
    reg.register(Box::new(MultiEditTool));
    reg.register(Box::new(GlobTool));
    reg.register(Box::new(GrepTool));
    reg.register(Box::new(LsTool));
    reg.register(Box::new(TodoWriteTool::new(todo_path.clone())));
    reg.register(Box::new(TodoReadTool::new(todo_path)));
    reg.register(Box::new(WebFetchTool::new(llm)));
    reg
}
```

- [ ] **Step 2: Rewrite `src/agent/context.rs`**

Full file:
```rust
use std::sync::{Arc, Mutex};
use crate::llm::{LLMClient, ModelTier};
use crate::agent::message::{Message, PromptCache};
use crate::memory::store::MemoryStore;
use crate::memory::index::MemoryIndex;
use crate::tools::ToolRegistry;
use crate::permissions::PermissionManager;

pub struct AgentContext {
    pub messages: Vec<Message>,
    pub prompt_cache: PromptCache,
    pub llm: Arc<LLMClient>,
    pub store: Arc<MemoryStore>,
    pub index: Arc<Mutex<MemoryIndex>>,
    pub config: AgentConfig,
    pub tool_registry: ToolRegistry,
    pub permission_manager: PermissionManager,
}

#[derive(Clone)]
pub struct AgentConfig {
    pub model_tier: ModelTier,
    pub max_iterations: usize,
    pub top_k_memory: usize,
    pub permission_mode: PermissionMode,
}

#[derive(Clone, PartialEq)]
pub enum PermissionMode {
    Default,
    Auto,
    Bypass,
}

impl Default for AgentConfig {
    fn default() -> Self {
        AgentConfig {
            model_tier: ModelTier::Medium,
            max_iterations: 50,
            top_k_memory: 5,
            permission_mode: PermissionMode::Default,
        }
    }
}

impl AgentContext {
    pub fn new(llm: Arc<LLMClient>, base_dir: std::path::PathBuf) -> crate::Result<Self> {
        let store = Arc::new(MemoryStore::new(base_dir)?);
        let index = Arc::new(Mutex::new(MemoryIndex::load(&store)?));
        let tool_registry = ToolRegistry::default_tools(Arc::clone(&llm));
        Ok(AgentContext {
            messages: vec![],
            prompt_cache: PromptCache::default(),
            llm,
            store,
            index,
            config: AgentConfig::default(),
            tool_registry,
            permission_manager: PermissionManager::default(),
        })
    }
}
```

- [ ] **Step 3: Rewrite `src/agent/run.rs`**

Full file:
```rust
use crate::Result;
use crate::agent::context::AgentContext;
use crate::agent::message::{ContentBlock, Message};
use crate::agent::prompt::build_system_prompt;
use crate::json::JsonValue;
use crate::memory::compaction::{compact_if_needed, estimate_tokens};
use crate::memory::retrieval::retrieve_relevant;

pub struct AgentOutput {
    pub text: String,
    pub iterations: usize,
}

pub fn run_agent(
    input: String,
    ctx: &mut AgentContext,
    ask_fn: &mut dyn FnMut(&str, &JsonValue) -> bool,
    mut on_text: impl FnMut(&str),
) -> Result<AgentOutput> {
    // 1. Retrieve relevant memories
    let memories = {
        let idx = ctx.index.lock().unwrap();
        retrieve_relevant(&input, &idx, &ctx.store, &ctx.llm, ctx.config.top_k_memory)?
    };

    // 2. Build system prompt (tools sent via API field, not system prompt)
    let system = build_system_prompt("", "", &memories, &mut ctx.prompt_cache);

    // 3. Append user message
    ctx.messages.push(Message::user_text(input));

    // 4. Compact context if needed
    let token_estimate = estimate_tokens(&ctx.messages);
    compact_if_needed(&mut ctx.messages, token_estimate, 100_000, 10, &ctx.llm)?;

    let tools_json = ctx.tool_registry.to_api_json();
    let mut final_text = String::new();
    let mut iterations = 0;

    loop {
        if iterations >= ctx.config.max_iterations { break; }
        iterations += 1;

        // 5. Call LLM with tools
        let stream_result = ctx.llm.stream_agent(
            &system.blocks,
            &ctx.messages,
            &tools_json,
            ctx.config.model_tier.clone(),
            &mut on_text,
        )?;

        // 6. Collect assistant blocks
        let mut assistant_blocks: Vec<ContentBlock> = stream_result.text_blocks.clone();
        assistant_blocks.extend(stream_result.tool_uses.clone());
        for b in &stream_result.text_blocks {
            if let ContentBlock::Text(t) = b { final_text = t.clone(); }
        }
        ctx.messages.push(Message::Assistant(assistant_blocks));

        // 7. Done if no tool calls
        if stream_result.tool_uses.is_empty() || stream_result.stop_reason == "end_turn" {
            break;
        }

        // 8. Execute tools with permission check
        let tool_results: Vec<ContentBlock> = stream_result.tool_uses.iter().map(|tu| {
            if let ContentBlock::ToolUse { id, name, input } = tu {
                let result = match ctx.tool_registry.get(name) {
                    None => Err(crate::Error::Tool(format!("unknown tool: {}", name))),
                    Some(tool) => {
                        if ctx.permission_manager.check(tool, input, ask_fn) {
                            tool.execute(input)
                        } else {
                            Err(crate::Error::Tool("permission denied by user".into()))
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

        // 9. Tool results as user message (Anthropic API requirement)
        ctx.messages.push(Message::User(tool_results));
    }

    Ok(AgentOutput { text: final_text, iterations })
}
```

- [ ] **Step 4: Build and verify clean**

```bash
cargo build 2>&1 | grep "^error"
```
Expected: no errors.

- [ ] **Step 5: Run all tests**

```bash
cargo test 2>&1 | tail -5
```
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/tools/mod.rs src/agent/context.rs src/agent/run.rs
git commit -m "feat(agent): real tool dispatch in run_agent with permission gating"
```

---

### Task 9: REPL ask_fn — permission prompt in TUI

**Files:**
- Modify: `src/repl.rs`

- [ ] **Step 1: Add `JsonValue` import to `src/repl.rs`**

Add to the imports block:
```rust
use crate::json::JsonValue;
```

- [ ] **Step 2: Replace the `run_agent` call site in `src/repl.rs`**

Find:
```rust
let agent_result =
    run_agent(line, &mut agent_ctx, "", "", |text| {
```

Replace with (construct `ask_fn` before the call, then pass it):
```rust
let mut ask_fn = |tool_name: &str, tool_input: &JsonValue| -> bool {
    use std::io::{Read, Write};
    let summary = format_tool_summary(tool_input);
    let prompt = format!(
        "\r\n\x1b[33m Allow {}({})? [y/n] \x1b[0m",
        tool_name, summary
    );
    let _ = std::io::stdout().write_all(prompt.as_bytes());
    let _ = std::io::stdout().flush();
    let mut buf = [0u8; 1];
    loop {
        match std::io::stdin().lock().read(&mut buf) {
            Ok(1) => match buf[0] {
                b'y' | b'Y' => {
                    let _ = std::io::stdout().write_all(b"y\r\n");
                    let _ = std::io::stdout().flush();
                    return true;
                }
                _ => {
                    let _ = std::io::stdout().write_all(b"n\r\n");
                    let _ = std::io::stdout().flush();
                    return false;
                }
            },
            _ => return false,
        }
    }
};

let agent_result =
    run_agent(line, &mut agent_ctx, &mut ask_fn, |text| {
```

- [ ] **Step 3: Add helper function `format_tool_summary` to `src/repl.rs`**

Add after the closing brace of `run()`:
```rust
fn format_tool_summary(input: &JsonValue) -> String {
    match input {
        JsonValue::Object(pairs) => pairs
            .iter()
            .take(2)
            .map(|(k, v)| {
                let val = match v {
                    JsonValue::Str(s) => format!("\"{}\"", s.chars().take(40).collect::<String>()),
                    other => format!("{}", other).chars().take(40).collect::<String>(),
                };
                format!("{}={}", k, val)
            })
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}
```

- [ ] **Step 4: Build and verify clean**

```bash
cargo build 2>&1 | grep "^error"
```
Expected: no errors.

- [ ] **Step 5: Run all tests**

```bash
cargo test 2>&1 | tail -5
```
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/repl.rs
git commit -m "feat(repl): inject ask_fn for tool permission prompts"
```
