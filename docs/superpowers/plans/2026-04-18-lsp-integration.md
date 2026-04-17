# LSP Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add built-in LSP client to viv, exposing 4 read-only tools (definition, references, hover, diagnostics) so the Agent can semantically understand code.

**Architecture:** Mirror the existing MCP module structure. Extend `StdioTransport` with Content-Length framing. `LspClient` wraps JSON-RPC over the transport. `LspManager` handles lazy server lifecycle. Four Tool implementations registered in `ToolRegistry`.

**Tech Stack:** Rust (edition 2024, zero dependencies), existing `jsonrpc.rs`, `StdioTransport`, `Tool` trait, `ToolRegistry`, `AssertSend`, `block_on`/`block_on_local`.

---

### Task 1: Extend StdioTransport with Content-Length framing

**Files:**
- Modify: `src/mcp/transport/stdio.rs`
- Test: `tests/mcp/transport_test.rs` (create)
- Modify: `tests/mcp/mod.rs`

- [ ] **Step 1: Write failing tests for Content-Length framing**

Create `tests/mcp/transport_test.rs`:

```rust
use viv::mcp::transport::stdio::Framing;

#[test]
fn content_length_encode() {
    let body = r#"{"jsonrpc":"2.0","method":"initialize","params":{}}"#;
    let encoded = Framing::ContentLength.encode(body);
    let expected = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
    assert_eq!(encoded, expected);
}

#[test]
fn newline_encode() {
    let body = r#"{"jsonrpc":"2.0","method":"initialize"}"#;
    let encoded = Framing::Newline.encode(body);
    let expected = format!("{}\n", body);
    assert_eq!(encoded, expected);
}

#[test]
fn content_length_decode_single() {
    let body = r#"{"jsonrpc":"2.0","id":1,"result":{}}"#;
    let raw = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
    let mut buf = raw.as_bytes().to_vec();
    let result = Framing::ContentLength.try_decode(&mut buf);
    assert_eq!(result, Some(body.to_string()));
    assert!(buf.is_empty());
}

#[test]
fn content_length_decode_partial_header() {
    let mut buf = b"Content-Len".to_vec();
    let result = Framing::ContentLength.try_decode(&mut buf);
    assert_eq!(result, None);
    assert_eq!(buf.len(), 11); // unchanged
}

#[test]
fn content_length_decode_partial_body() {
    let body = r#"{"jsonrpc":"2.0","id":1,"result":{}}"#;
    let raw = format!("Content-Length: {}\r\n\r\n{}", body.len(), &body[..5]);
    let mut buf = raw.as_bytes().to_vec();
    let result = Framing::ContentLength.try_decode(&mut buf);
    assert_eq!(result, None);
}

#[test]
fn newline_decode_single() {
    let body = r#"{"jsonrpc":"2.0","id":1,"result":{}}"#;
    let raw = format!("{}\n", body);
    let mut buf = raw.as_bytes().to_vec();
    let result = Framing::Newline.try_decode(&mut buf);
    assert_eq!(result, Some(body.to_string()));
    assert!(buf.is_empty());
}
```

Add `pub mod transport_test;` to `tests/mcp/mod.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test mcp_tests transport_test`
Expected: compilation error — `Framing` does not exist.

- [ ] **Step 3: Implement Framing enum with encode/try_decode**

In `src/mcp/transport/stdio.rs`, add above `StdioTransport`:

```rust
/// Message framing mode for stdio transport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Framing {
    /// Newline-delimited JSON (used by MCP).
    Newline,
    /// Content-Length header framing (used by LSP).
    ContentLength,
}

impl Framing {
    /// Encode a JSON string for sending.
    pub fn encode(&self, body: &str) -> String {
        match self {
            Framing::Newline => format!("{}\n", body),
            Framing::ContentLength => {
                format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
            }
        }
    }

    /// Try to decode one complete message from the buffer.
    /// Returns `Some(body)` and drains consumed bytes, or `None` if incomplete.
    pub fn try_decode(&self, buf: &mut Vec<u8>) -> Option<String> {
        match self {
            Framing::Newline => {
                let pos = buf.iter().position(|&b| b == b'\n')?;
                let line: Vec<u8> = buf.drain(..=pos).collect();
                let s = std::str::from_utf8(&line[..line.len() - 1]).ok()?;
                if s.is_empty() {
                    return None;
                }
                Some(s.to_string())
            }
            Framing::ContentLength => {
                let header_end = find_header_end(buf)?;
                let header = std::str::from_utf8(&buf[..header_end]).ok()?;
                let content_length = parse_content_length(header)?;
                let total = header_end + 4 + content_length; // header + \r\n\r\n + body
                if buf.len() < total {
                    return None;
                }
                let body_start = header_end + 4;
                let body = std::str::from_utf8(&buf[body_start..body_start + content_length])
                    .ok()?
                    .to_string();
                buf.drain(..total);
                Some(body)
            }
        }
    }
}

/// Find the position of \r\n\r\n in the buffer (returns position of first \r).
fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

/// Parse "Content-Length: N" from header string.
fn parse_content_length(header: &str) -> Option<usize> {
    for line in header.split("\r\n") {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("Content-Length:") {
            return val.trim().parse().ok();
        }
    }
    None
}
```

- [ ] **Step 4: Add framing field to StdioTransport and refactor send/recv**

Add `framing: Framing` field to `StdioTransport`. Update `spawn()` to default to `Framing::Newline`. Add `spawn_lsp()` that uses `Framing::ContentLength`.

```rust
pub struct StdioTransport {
    child: Child,
    stdin_fd: RawFd,
    stdout_fd: RawFd,
    read_buf: Vec<u8>,
    framing: Framing,
}
```

Update `spawn()`:
```rust
pub fn spawn(
    command: &str,
    args: &[&str],
    env: &HashMap<String, String>,
) -> crate::Result<Self> {
    Self::spawn_with_framing(command, args, env, Framing::Newline)
}

pub fn spawn_with_framing(
    command: &str,
    args: &[&str],
    env: &HashMap<String, String>,
    framing: Framing,
) -> crate::Result<Self> {
    // ... same spawn logic ...
    Ok(StdioTransport {
        child,
        stdin_fd: stdin_raw,
        stdout_fd: stdout_raw,
        read_buf: Vec::with_capacity(4096),
        framing,
    })
}
```

Refactor `send()` in `impl Transport`:
```rust
fn send(&mut self, msg: JsonValue) -> Pin<Box<dyn Future<Output = crate::Result<()>> + Send + '_>> {
    Box::pin(async move {
        let data = self.framing.encode(&format!("{}", msg));
        let bytes = data.as_bytes();
        let mut written = 0;
        while written < bytes.len() {
            let n = unsafe {
                write(self.stdin_fd, bytes[written..].as_ptr(), bytes[written..].len())
            };
            if n < 0 {
                let errno = unsafe { *__errno_location() };
                if errno == EAGAIN || errno == EWOULDBLOCK {
                    std::thread::yield_now();
                    continue;
                }
                return Err(crate::Error::Io(std::io::Error::from_raw_os_error(errno)));
            }
            written += n as usize;
        }
        Ok(())
    })
}
```

Refactor `recv()` in `impl Transport`:
```rust
fn recv(&mut self) -> Pin<Box<dyn Future<Output = crate::Result<JsonValue>> + Send + '_>> {
    Box::pin(async move {
        loop {
            if let Some(json_str) = self.framing.try_decode(&mut self.read_buf) {
                return JsonValue::parse(&json_str);
            }
            wait_readable(self.stdout_fd).await?;
            let mut tmp = [0u8; 4096];
            let n = unsafe { read(self.stdout_fd, tmp.as_mut_ptr(), tmp.len()) };
            if n > 0 {
                self.read_buf.extend_from_slice(&tmp[..n as usize]);
            } else if n == 0 {
                return Err(crate::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "server closed connection",
                )));
            } else {
                let errno = unsafe { *__errno_location() };
                if errno == EAGAIN || errno == EWOULDBLOCK {
                    continue;
                }
                return Err(crate::Error::Io(std::io::Error::from_raw_os_error(errno)));
            }
        }
    })
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test mcp_tests transport_test`
Expected: all 6 tests PASS.

- [ ] **Step 6: Run full test suite to check for regressions**

Run: `cargo test`
Expected: all existing tests PASS (MCP behavior unchanged).

- [ ] **Step 7: Commit**

```bash
git add src/mcp/transport/stdio.rs tests/mcp/transport_test.rs tests/mcp/mod.rs
git commit -m "feat(transport): add Content-Length framing to StdioTransport for LSP"
```

---

### Task 2: Add Error::Lsp variant

**Files:**
- Modify: `src/error.rs`

- [ ] **Step 1: Add Lsp variant to Error enum**

In `src/error.rs`, add after the `Mcp` variant:

```rust
/// LSP runtime errors
Lsp { server: String, message: String },
```

Add the Display match arm:

```rust
Error::Lsp { server, message } => write!(f, "LSP error [{}]: {}", server, message),
```

- [ ] **Step 2: Run tests to verify no regressions**

Run: `cargo test`
Expected: all tests PASS.

- [ ] **Step 3: Commit**

```bash
git add src/error.rs
git commit -m "feat(error): add Lsp variant to Error enum"
```

---

### Task 3: LSP types

**Files:**
- Create: `src/lsp/types.rs`

- [ ] **Step 1: Write failing tests for LSP type parsing**

Create `tests/lsp/types_test.rs`:

```rust
use viv::core::json::JsonValue;
use viv::lsp::types::*;

#[test]
fn parse_position() {
    let json = JsonValue::Object(vec![
        ("line".into(), JsonValue::Number(viv::core::json::Number::Int(10))),
        ("character".into(), JsonValue::Number(viv::core::json::Number::Int(5))),
    ]);
    let pos = Position::from_json(&json).unwrap();
    assert_eq!(pos.line, 10);
    assert_eq!(pos.character, 5);
}

#[test]
fn parse_location() {
    let json = JsonValue::Object(vec![
        ("uri".into(), JsonValue::Str("file:///src/main.rs".into())),
        ("range".into(), JsonValue::Object(vec![
            ("start".into(), JsonValue::Object(vec![
                ("line".into(), JsonValue::Number(viv::core::json::Number::Int(10))),
                ("character".into(), JsonValue::Number(viv::core::json::Number::Int(0))),
            ])),
            ("end".into(), JsonValue::Object(vec![
                ("line".into(), JsonValue::Number(viv::core::json::Number::Int(10))),
                ("character".into(), JsonValue::Number(viv::core::json::Number::Int(20))),
            ])),
        ])),
    ]);
    let loc = Location::from_json(&json).unwrap();
    assert_eq!(loc.uri, "file:///src/main.rs");
    assert_eq!(loc.range.start.line, 10);
    assert_eq!(loc.range.end.character, 20);
}

#[test]
fn position_to_json() {
    let pos = Position { line: 5, character: 3 };
    let json = pos.to_json();
    assert_eq!(json.get("line").unwrap().as_i64(), Some(5));
    assert_eq!(json.get("character").unwrap().as_i64(), Some(3));
}

#[test]
fn parse_hover_result_with_string() {
    let json = JsonValue::Object(vec![
        ("contents".into(), JsonValue::Str("fn main()".into())),
    ]);
    let hover = HoverResult::from_json(&json).unwrap();
    assert_eq!(hover.contents, "fn main()");
}

#[test]
fn parse_hover_result_with_markup() {
    let json = JsonValue::Object(vec![
        ("contents".into(), JsonValue::Object(vec![
            ("kind".into(), JsonValue::Str("markdown".into())),
            ("value".into(), JsonValue::Str("```rust\nfn main()\n```".into())),
        ])),
    ]);
    let hover = HoverResult::from_json(&json).unwrap();
    assert_eq!(hover.contents, "```rust\nfn main()\n```");
}

#[test]
fn parse_diagnostic() {
    let json = JsonValue::Object(vec![
        ("range".into(), JsonValue::Object(vec![
            ("start".into(), JsonValue::Object(vec![
                ("line".into(), JsonValue::Number(viv::core::json::Number::Int(12))),
                ("character".into(), JsonValue::Number(viv::core::json::Number::Int(4))),
            ])),
            ("end".into(), JsonValue::Object(vec![
                ("line".into(), JsonValue::Number(viv::core::json::Number::Int(12))),
                ("character".into(), JsonValue::Number(viv::core::json::Number::Int(10))),
            ])),
        ])),
        ("severity".into(), JsonValue::Number(viv::core::json::Number::Int(1))),
        ("message".into(), JsonValue::Str("unused variable".into())),
    ]);
    let diag = Diagnostic::from_json(&json).unwrap();
    assert_eq!(diag.range.start.line, 12);
    assert_eq!(diag.severity, DiagnosticSeverity::Error);
    assert_eq!(diag.message, "unused variable");
}

#[test]
fn parse_server_capabilities() {
    let json = JsonValue::Object(vec![
        ("capabilities".into(), JsonValue::Object(vec![
            ("definitionProvider".into(), JsonValue::Bool(true)),
            ("referencesProvider".into(), JsonValue::Bool(true)),
            ("hoverProvider".into(), JsonValue::Bool(true)),
        ])),
    ]);
    let caps = LspServerCapabilities::from_json(&json).unwrap();
    assert!(caps.definition);
    assert!(caps.references);
    assert!(caps.hover);
}
```

Create `tests/lsp/mod.rs`:
```rust
pub mod types_test;
```

Create `tests/lsp_tests.rs`:
```rust
mod lsp;
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test lsp_tests types_test`
Expected: compilation error — `viv::lsp` module does not exist.

- [ ] **Step 3: Create lsp module and types**

Create `src/lsp/mod.rs`:
```rust
pub mod types;
```

Add `pub mod lsp;` to `src/lib.rs`.

Create `src/lsp/types.rs`:

```rust
use crate::core::json::{JsonValue, Number};
use crate::{Error, Result};

#[derive(Debug, Clone)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

impl Position {
    pub fn from_json(json: &JsonValue) -> Result<Self> {
        let line = json.get("line").and_then(|v| v.as_i64())
            .ok_or_else(|| Error::Json("Position: missing 'line'".into()))? as u32;
        let character = json.get("character").and_then(|v| v.as_i64())
            .ok_or_else(|| Error::Json("Position: missing 'character'".into()))? as u32;
        Ok(Position { line, character })
    }

    pub fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("line".into(), JsonValue::Number(Number::Int(self.line as i64))),
            ("character".into(), JsonValue::Number(Number::Int(self.character as i64))),
        ])
    }
}

#[derive(Debug, Clone)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub fn from_json(json: &JsonValue) -> Result<Self> {
        let start = Position::from_json(
            json.get("start").ok_or_else(|| Error::Json("Range: missing 'start'".into()))?
        )?;
        let end = Position::from_json(
            json.get("end").ok_or_else(|| Error::Json("Range: missing 'end'".into()))?
        )?;
        Ok(Range { start, end })
    }
}

#[derive(Debug, Clone)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

impl Location {
    pub fn from_json(json: &JsonValue) -> Result<Self> {
        let uri = json.get("uri").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Json("Location: missing 'uri'".into()))?.to_string();
        let range = Range::from_json(
            json.get("range").ok_or_else(|| Error::Json("Location: missing 'range'".into()))?
        )?;
        Ok(Location { uri, range })
    }

    /// Convert file:// URI to a local path.
    pub fn file_path(&self) -> Option<&str> {
        self.uri.strip_prefix("file://")
    }
}

#[derive(Debug, Clone)]
pub struct HoverResult {
    pub contents: String,
}

impl HoverResult {
    pub fn from_json(json: &JsonValue) -> Result<Self> {
        let contents_val = json.get("contents")
            .ok_or_else(|| Error::Json("Hover: missing 'contents'".into()))?;
        let contents = match contents_val {
            JsonValue::Str(s) => s.clone(),
            JsonValue::Object(_) => {
                // MarkupContent: { kind, value }
                contents_val.get("value").and_then(|v| v.as_str())
                    .unwrap_or("").to_string()
            }
            JsonValue::Array(arr) => {
                // MarkedString[]
                arr.iter().filter_map(|item| match item {
                    JsonValue::Str(s) => Some(s.clone()),
                    JsonValue::Object(_) => item.get("value").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    _ => None,
                }).collect::<Vec<_>>().join("\n")
            }
            _ => String::new(),
        };
        Ok(HoverResult { contents })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DiagnosticSeverity {
    Error,       // 1
    Warning,     // 2
    Information, // 3
    Hint,        // 4
}

impl DiagnosticSeverity {
    pub fn from_i64(v: i64) -> Self {
        match v {
            1 => DiagnosticSeverity::Error,
            2 => DiagnosticSeverity::Warning,
            3 => DiagnosticSeverity::Information,
            _ => DiagnosticSeverity::Hint,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            DiagnosticSeverity::Error => "error",
            DiagnosticSeverity::Warning => "warning",
            DiagnosticSeverity::Information => "info",
            DiagnosticSeverity::Hint => "hint",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub code: Option<String>,
}

impl Diagnostic {
    pub fn from_json(json: &JsonValue) -> Result<Self> {
        let range = Range::from_json(
            json.get("range").ok_or_else(|| Error::Json("Diagnostic: missing 'range'".into()))?
        )?;
        let severity = json.get("severity").and_then(|v| v.as_i64())
            .map(DiagnosticSeverity::from_i64)
            .unwrap_or(DiagnosticSeverity::Error);
        let message = json.get("message").and_then(|v| v.as_str())
            .ok_or_else(|| Error::Json("Diagnostic: missing 'message'".into()))?.to_string();
        let code = json.get("code").and_then(|v| match v {
            JsonValue::Str(s) => Some(s.clone()),
            JsonValue::Number(n) => Some(format!("{}", n)),
            _ => None,
        });
        Ok(Diagnostic { range, severity, message, code })
    }
}

#[derive(Debug, Clone)]
pub struct LspServerCapabilities {
    pub definition: bool,
    pub references: bool,
    pub hover: bool,
}

impl LspServerCapabilities {
    pub fn from_json(json: &JsonValue) -> Result<Self> {
        let caps = json.get("capabilities").unwrap_or(json);
        Ok(LspServerCapabilities {
            definition: caps.get("definitionProvider").and_then(|v| v.as_bool()).unwrap_or(false),
            references: caps.get("referencesProvider").and_then(|v| v.as_bool()).unwrap_or(false),
            hover: caps.get("hoverProvider").and_then(|v| v.as_bool()).unwrap_or(false),
        })
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test lsp_tests types_test`
Expected: all 7 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lsp/ src/lib.rs tests/lsp/ tests/lsp_tests.rs
git commit -m "feat(lsp): add LSP type definitions with JSON parsing"
```

---

### Task 4: LSP config parsing

**Files:**
- Create: `src/lsp/config.rs`
- Modify: `src/lsp/mod.rs`

- [ ] **Step 1: Write failing tests for LSP config**

Create `tests/lsp/config_test.rs`:

```rust
use viv::lsp::config::LspConfig;

#[test]
fn parse_single_server() {
    let json = r#"{
        "lspServers": {
            "rust": {
                "command": "rust-analyzer",
                "args": [],
                "extensions": [".rs"]
            }
        }
    }"#;
    let config = LspConfig::parse(json).unwrap();
    assert_eq!(config.servers.len(), 1);

    let (name, server) = &config.servers[0];
    assert_eq!(name, "rust");
    assert_eq!(server.command, "rust-analyzer");
    assert!(server.args.is_empty());
    assert_eq!(server.extensions, vec![".rs"]);
}

#[test]
fn parse_multiple_servers() {
    let json = r#"{
        "lspServers": {
            "rust": {
                "command": "rust-analyzer",
                "extensions": [".rs"]
            },
            "python": {
                "command": "pylsp",
                "args": ["--check-parent-process"],
                "extensions": [".py", ".pyi"]
            }
        }
    }"#;
    let config = LspConfig::parse(json).unwrap();
    assert_eq!(config.servers.len(), 2);

    let py = config.servers.iter().find(|(n, _)| n == "python").unwrap();
    assert_eq!(py.1.command, "pylsp");
    assert_eq!(py.1.args, vec!["--check-parent-process"]);
    assert_eq!(py.1.extensions, vec![".py", ".pyi"]);
}

#[test]
fn empty_config() {
    let config = LspConfig::parse("{}").unwrap();
    assert!(config.servers.is_empty());
}

#[test]
fn load_nonexistent_file() {
    let config = LspConfig::load("/tmp/viv_test_nonexistent_lsp/settings.json").unwrap();
    assert!(config.servers.is_empty());
}

#[test]
fn server_for_extension() {
    let json = r#"{
        "lspServers": {
            "rust": { "command": "rust-analyzer", "extensions": [".rs"] },
            "python": { "command": "pylsp", "extensions": [".py"] }
        }
    }"#;
    let config = LspConfig::parse(json).unwrap();
    assert_eq!(config.server_for_extension(".rs"), Some("rust"));
    assert_eq!(config.server_for_extension(".py"), Some("python"));
    assert_eq!(config.server_for_extension(".js"), None);
}
```

Add `pub mod config_test;` to `tests/lsp/mod.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test lsp_tests config_test`
Expected: compilation error — `viv::lsp::config` does not exist.

- [ ] **Step 3: Implement LspConfig**

Create `src/lsp/config.rs`:

```rust
use crate::core::json::JsonValue;
use crate::{Error, Result};
use std::fs;

pub struct LspServerConfig {
    pub command: String,
    pub args: Vec<String>,
    pub extensions: Vec<String>,
    pub env: Vec<(String, String)>,
}

pub struct LspConfig {
    pub servers: Vec<(String, LspServerConfig)>,
}

impl LspConfig {
    pub fn load(path: &str) -> Result<Self> {
        let contents = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(LspConfig { servers: vec![] });
            }
            Err(e) => return Err(Error::Io(e)),
        };
        Self::parse(&contents)
    }

    pub fn parse(json_str: &str) -> Result<Self> {
        let root = JsonValue::parse(json_str)?;
        let servers_obj = match root.get("lspServers") {
            Some(v) => v,
            None => return Ok(LspConfig { servers: vec![] }),
        };
        let pairs = servers_obj
            .as_object()
            .ok_or_else(|| Error::Json("lspServers must be an object".into()))?;

        let mut servers = Vec::with_capacity(pairs.len());
        for (name, value) in pairs {
            let server = parse_lsp_server(name, value)?;
            servers.push((name.clone(), server));
        }
        Ok(LspConfig { servers })
    }

    /// Find which server handles a given file extension.
    pub fn server_for_extension(&self, ext: &str) -> Option<&str> {
        self.servers
            .iter()
            .find(|(_, s)| s.extensions.iter().any(|e| e == ext))
            .map(|(name, _)| name.as_str())
    }
}

fn parse_lsp_server(name: &str, value: &JsonValue) -> Result<LspServerConfig> {
    let command = value
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::Json(format!("LSP server '{}': missing 'command'", name)))?
        .to_string();

    let args = match value.get("args") {
        Some(arr_val) => {
            let arr = arr_val
                .as_array()
                .ok_or_else(|| Error::Json(format!("LSP server '{}': 'args' must be array", name)))?;
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        }
        None => vec![],
    };

    let extensions = match value.get("extensions") {
        Some(arr_val) => {
            let arr = arr_val
                .as_array()
                .ok_or_else(|| Error::Json(format!("LSP server '{}': 'extensions' must be array", name)))?;
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        }
        None => vec![],
    };

    let env = match value.get("env") {
        Some(env_val) => {
            let pairs = env_val
                .as_object()
                .ok_or_else(|| Error::Json(format!("LSP server '{}': 'env' must be object", name)))?;
            pairs
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        }
        None => vec![],
    };

    Ok(LspServerConfig { command, args, extensions, env })
}
```

Add `pub mod config;` to `src/lsp/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test lsp_tests config_test`
Expected: all 5 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lsp/config.rs src/lsp/mod.rs tests/lsp/config_test.rs tests/lsp/mod.rs
git commit -m "feat(lsp): add LSP config parsing from settings.json"
```

---

### Task 5: LspClient

**Files:**
- Create: `src/lsp/client.rs`
- Modify: `src/lsp/mod.rs`

- [ ] **Step 1: Write failing tests for LspClient**

Create `tests/lsp/client_test.rs`:

```rust
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;

use viv::core::json::{JsonValue, Number};
use viv::core::runtime::block_on;
use viv::mcp::transport::Transport;
use viv::lsp::client::LspClient;

struct MockTransport {
    sent: Vec<JsonValue>,
    responses: VecDeque<JsonValue>,
}

impl MockTransport {
    fn new(responses: Vec<JsonValue>) -> Self {
        MockTransport { sent: Vec::new(), responses: VecDeque::from(responses) }
    }
}

impl Transport for MockTransport {
    fn send(&mut self, msg: JsonValue) -> Pin<Box<dyn Future<Output = viv::Result<()>> + Send + '_>> {
        self.sent.push(msg);
        Box::pin(async { Ok(()) })
    }
    fn recv(&mut self) -> Pin<Box<dyn Future<Output = viv::Result<JsonValue>> + Send + '_>> {
        let response = self.responses.pop_front()
            .ok_or_else(|| viv::Error::Lsp { server: "mock".into(), message: "no response".into() });
        Box::pin(async move { response })
    }
    fn close(&mut self) -> Pin<Box<dyn Future<Output = viv::Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }
}

fn success(id: i64, result: JsonValue) -> JsonValue {
    JsonValue::Object(vec![
        ("jsonrpc".into(), JsonValue::Str("2.0".into())),
        ("id".into(), JsonValue::Number(Number::Int(id))),
        ("result".into(), result),
    ])
}

fn lsp_init_result() -> JsonValue {
    JsonValue::Object(vec![
        ("capabilities".into(), JsonValue::Object(vec![
            ("definitionProvider".into(), JsonValue::Bool(true)),
            ("referencesProvider".into(), JsonValue::Bool(true)),
            ("hoverProvider".into(), JsonValue::Bool(true)),
        ])),
    ])
}

#[test]
fn initialize_handshake() {
    let transport = MockTransport::new(vec![success(1, lsp_init_result())]);
    let mut client = LspClient::new(transport, "test");

    let (caps, client) = block_on(async move {
        let caps = client.initialize("/workspace").await.unwrap();
        (caps, client)
    });

    assert!(caps.definition);
    assert!(caps.references);
    assert!(caps.hover);

    let transport = client.into_transport();
    // initialize request + initialized notification
    assert_eq!(transport.sent.len(), 2);
    assert_eq!(transport.sent[0].get("method").unwrap().as_str().unwrap(), "initialize");
    assert_eq!(transport.sent[1].get("method").unwrap().as_str().unwrap(), "initialized");
    // initialized is a notification (no id)
    assert!(transport.sent[1].get("id").is_none());
}

#[test]
fn definition_request() {
    let result = JsonValue::Array(vec![
        JsonValue::Object(vec![
            ("uri".into(), JsonValue::Str("file:///src/main.rs".into())),
            ("range".into(), JsonValue::Object(vec![
                ("start".into(), JsonValue::Object(vec![
                    ("line".into(), JsonValue::Number(Number::Int(10))),
                    ("character".into(), JsonValue::Number(Number::Int(0))),
                ])),
                ("end".into(), JsonValue::Object(vec![
                    ("line".into(), JsonValue::Number(Number::Int(10))),
                    ("character".into(), JsonValue::Number(Number::Int(20))),
                ])),
            ])),
        ]),
    ]);

    let transport = MockTransport::new(vec![success(1, result)]);
    let mut client = LspClient::new(transport, "test");

    let locations = block_on(async move {
        client.definition("file:///src/lib.rs", 5, 10).await.unwrap()
    });

    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0].uri, "file:///src/main.rs");
    assert_eq!(locations[0].range.start.line, 10);
}

#[test]
fn hover_request() {
    let result = JsonValue::Object(vec![
        ("contents".into(), JsonValue::Object(vec![
            ("kind".into(), JsonValue::Str("markdown".into())),
            ("value".into(), JsonValue::Str("```rust\nfn main()\n```".into())),
        ])),
    ]);

    let transport = MockTransport::new(vec![success(1, result)]);
    let mut client = LspClient::new(transport, "test");

    let hover = block_on(async move {
        client.hover("file:///src/main.rs", 1, 3).await.unwrap()
    });

    assert!(hover.is_some());
    assert!(hover.unwrap().contents.contains("fn main()"));
}

#[test]
fn diagnostics_from_notification() {
    // Server sends a publishDiagnostics notification before our response
    let notif = JsonValue::Object(vec![
        ("jsonrpc".into(), JsonValue::Str("2.0".into())),
        ("method".into(), JsonValue::Str("textDocument/publishDiagnostics".into())),
        ("params".into(), JsonValue::Object(vec![
            ("uri".into(), JsonValue::Str("file:///src/main.rs".into())),
            ("diagnostics".into(), JsonValue::Array(vec![
                JsonValue::Object(vec![
                    ("range".into(), JsonValue::Object(vec![
                        ("start".into(), JsonValue::Object(vec![
                            ("line".into(), JsonValue::Number(Number::Int(5))),
                            ("character".into(), JsonValue::Number(Number::Int(0))),
                        ])),
                        ("end".into(), JsonValue::Object(vec![
                            ("line".into(), JsonValue::Number(Number::Int(5))),
                            ("character".into(), JsonValue::Number(Number::Int(10))),
                        ])),
                    ])),
                    ("severity".into(), JsonValue::Number(Number::Int(1))),
                    ("message".into(), JsonValue::Str("unused variable".into())),
                ]),
            ])),
        ])),
    ]);

    let hover_result = JsonValue::Object(vec![
        ("contents".into(), JsonValue::Str("type info".into())),
    ]);

    let transport = MockTransport::new(vec![notif, success(1, hover_result)]);
    let mut client = LspClient::new(transport, "test");

    let (hover, diags) = block_on(async move {
        let hover = client.hover("file:///src/main.rs", 1, 0).await.unwrap();
        let diags = client.diagnostics("file:///src/main.rs");
        (hover, diags)
    });

    assert!(hover.is_some());
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].message, "unused variable");
}

#[test]
fn null_result_for_hover() {
    let transport = MockTransport::new(vec![success(1, JsonValue::Null)]);
    let mut client = LspClient::new(transport, "test");
    let hover = block_on(async move { client.hover("file:///f.rs", 0, 0).await.unwrap() });
    assert!(hover.is_none());
}
```

Add `pub mod client_test;` to `tests/lsp/mod.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test lsp_tests client_test`
Expected: compilation error — `viv::lsp::client` does not exist.

- [ ] **Step 3: Implement LspClient**

Create `src/lsp/client.rs`:

```rust
use std::collections::HashMap;
use crate::Error;
use crate::core::json::{JsonValue, Number};
use crate::core::jsonrpc::{Message, Notification, Request, Response};
use crate::lsp::types::*;
use crate::mcp::transport::Transport;

pub struct LspClient<T: Transport> {
    transport: T,
    next_id: i64,
    server_name: String,
    pub capabilities: Option<LspServerCapabilities>,
    diagnostics_cache: HashMap<String, Vec<Diagnostic>>,
}

impl<T: Transport> LspClient<T> {
    pub fn new(transport: T, server_name: &str) -> Self {
        LspClient {
            transport,
            next_id: 1,
            server_name: server_name.to_string(),
            capabilities: None,
            diagnostics_cache: HashMap::new(),
        }
    }

    async fn request(&mut self, method: &str, params: Option<JsonValue>) -> crate::Result<JsonValue> {
        let id = self.next_id;
        self.next_id += 1;
        let req = Request { id, method: method.to_string(), params };
        self.transport.send(req.to_json()).await?;
        loop {
            let msg_json = self.transport.recv().await?;
            let msg = Message::parse(&msg_json)?;
            match msg {
                Message::Response(Response::Result { id: rid, result }) if rid == id => {
                    return Ok(result);
                }
                Message::Response(Response::Error { id: rid, error }) if rid == id => {
                    return Err(Error::JsonRpc { code: error.code, message: error.message });
                }
                Message::Notification(n) => {
                    self.handle_notification(&n);
                }
                _ => {}
            }
        }
    }

    async fn notify(&mut self, method: &str, params: Option<JsonValue>) -> crate::Result<()> {
        let notif = Notification { method: method.to_string(), params };
        self.transport.send(notif.to_json()).await
    }

    fn handle_notification(&mut self, notif: &Notification) {
        if notif.method == "textDocument/publishDiagnostics" {
            if let Some(params) = &notif.params {
                if let Some(uri) = params.get("uri").and_then(|v| v.as_str()) {
                    let diags = params
                        .get("diagnostics")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|d| Diagnostic::from_json(d).ok())
                                .collect()
                        })
                        .unwrap_or_default();
                    self.diagnostics_cache.insert(uri.to_string(), diags);
                }
            }
        }
    }

    pub async fn initialize(&mut self, root_path: &str) -> crate::Result<LspServerCapabilities> {
        let root_uri = if root_path.starts_with("file://") {
            root_path.to_string()
        } else {
            format!("file://{}", root_path)
        };

        let params = JsonValue::Object(vec![
            ("processId".into(), JsonValue::Number(Number::Int(std::process::id() as i64))),
            ("rootUri".into(), JsonValue::Str(root_uri)),
            ("capabilities".into(), JsonValue::Object(vec![
                ("textDocument".into(), JsonValue::Object(vec![
                    ("definition".into(), JsonValue::Object(vec![])),
                    ("references".into(), JsonValue::Object(vec![])),
                    ("hover".into(), JsonValue::Object(vec![
                        ("contentFormat".into(), JsonValue::Array(vec![
                            JsonValue::Str("markdown".into()),
                            JsonValue::Str("plaintext".into()),
                        ])),
                    ])),
                    ("publishDiagnostics".into(), JsonValue::Object(vec![])),
                ])),
            ])),
        ]);

        let result = self.request("initialize", Some(params)).await?;
        let caps = LspServerCapabilities::from_json(&result)?;
        self.capabilities = Some(caps.clone());
        self.notify("initialized", Some(JsonValue::Object(vec![]))).await?;
        Ok(caps)
    }

    pub async fn definition(&mut self, uri: &str, line: u32, character: u32) -> crate::Result<Vec<Location>> {
        let params = text_document_position(uri, line, character);
        let result = self.request("textDocument/definition", Some(params)).await?;
        parse_location_response(&result)
    }

    pub async fn references(&mut self, uri: &str, line: u32, character: u32) -> crate::Result<Vec<Location>> {
        let mut params = text_document_position(uri, line, character);
        // LSP requires context.includeDeclaration
        if let JsonValue::Object(ref mut pairs) = params {
            pairs.push(("context".into(), JsonValue::Object(vec![
                ("includeDeclaration".into(), JsonValue::Bool(true)),
            ])));
        }
        let result = self.request("textDocument/references", Some(params)).await?;
        parse_location_response(&result)
    }

    pub async fn hover(&mut self, uri: &str, line: u32, character: u32) -> crate::Result<Option<HoverResult>> {
        let params = text_document_position(uri, line, character);
        let result = self.request("textDocument/hover", Some(params)).await?;
        if result == JsonValue::Null {
            return Ok(None);
        }
        Ok(Some(HoverResult::from_json(&result)?))
    }

    pub fn diagnostics(&self, uri: &str) -> Vec<Diagnostic> {
        self.diagnostics_cache.get(uri).cloned().unwrap_or_default()
    }

    pub fn all_diagnostics(&self) -> &HashMap<String, Vec<Diagnostic>> {
        &self.diagnostics_cache
    }

    pub async fn did_open(&mut self, uri: &str, language_id: &str, text: &str) -> crate::Result<()> {
        let params = JsonValue::Object(vec![
            ("textDocument".into(), JsonValue::Object(vec![
                ("uri".into(), JsonValue::Str(uri.to_string())),
                ("languageId".into(), JsonValue::Str(language_id.to_string())),
                ("version".into(), JsonValue::Number(Number::Int(1))),
                ("text".into(), JsonValue::Str(text.to_string())),
            ])),
        ]);
        self.notify("textDocument/didOpen", Some(params)).await
    }

    pub async fn shutdown(&mut self) -> crate::Result<()> {
        let _ = self.request("shutdown", None).await;
        self.notify("exit", None).await?;
        self.transport.close().await
    }

    pub fn into_transport(self) -> T {
        self.transport
    }
}

fn text_document_position(uri: &str, line: u32, character: u32) -> JsonValue {
    JsonValue::Object(vec![
        ("textDocument".into(), JsonValue::Object(vec![
            ("uri".into(), JsonValue::Str(uri.to_string())),
        ])),
        ("position".into(), Position { line, character }.to_json()),
    ])
}

fn parse_location_response(result: &JsonValue) -> crate::Result<Vec<Location>> {
    match result {
        JsonValue::Null => Ok(vec![]),
        JsonValue::Array(arr) => {
            arr.iter().map(|v| Location::from_json(v)).collect()
        }
        JsonValue::Object(_) => {
            Ok(vec![Location::from_json(result)?])
        }
        _ => Ok(vec![]),
    }
}
```

Add `pub mod client;` to `src/lsp/mod.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test lsp_tests client_test`
Expected: all 5 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lsp/client.rs src/lsp/mod.rs tests/lsp/client_test.rs tests/lsp/mod.rs
git commit -m "feat(lsp): add LspClient with initialize, definition, references, hover, diagnostics"
```

---

### Task 6: LspManager

**Files:**
- Modify: `src/lsp/mod.rs`

- [ ] **Step 1: Write failing tests for LspManager**

Create `tests/lsp/manager_test.rs`:

```rust
use viv::lsp::config::LspConfig;
use viv::lsp::LspManager;

#[test]
fn empty_config_creates_empty_manager() {
    let config = LspConfig::parse("{}").unwrap();
    let mgr = LspManager::new(config);
    assert!(mgr.is_empty());
}

#[test]
fn server_for_file_resolves_by_extension() {
    let json = r#"{
        "lspServers": {
            "rust": { "command": "rust-analyzer", "extensions": [".rs"] },
            "python": { "command": "pylsp", "extensions": [".py"] }
        }
    }"#;
    let config = LspConfig::parse(json).unwrap();
    let mgr = LspManager::new(config);
    assert_eq!(mgr.server_name_for_file("src/main.rs"), Some("rust"));
    assert_eq!(mgr.server_name_for_file("app.py"), Some("python"));
    assert_eq!(mgr.server_name_for_file("style.css"), None);
}
```

Add `pub mod manager_test;` to `tests/lsp/mod.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test lsp_tests manager_test`
Expected: compilation error — `LspManager` does not exist.

- [ ] **Step 3: Implement LspManager**

Replace `src/lsp/mod.rs` contents:

```rust
pub mod client;
pub mod config;
pub mod types;
pub mod tools;

use std::collections::HashMap;
use std::path::Path;

use crate::Error;
use crate::mcp::transport::stdio::{Framing, StdioTransport};
use client::LspClient;
use config::{LspConfig, LspServerConfig};

type LspStdioClient = LspClient<StdioTransport>;

pub struct LspManager {
    config: LspConfig,
    servers: HashMap<String, Option<LspStdioClient>>,
    opened_files: std::collections::HashSet<String>,
}

impl LspManager {
    pub fn new(config: LspConfig) -> Self {
        let mut servers = HashMap::new();
        for (name, _) in &config.servers {
            servers.insert(name.clone(), None);
        }
        LspManager {
            config,
            servers,
            opened_files: std::collections::HashSet::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.servers.is_empty()
    }

    /// Find the server name for a file path, based on extension mapping.
    pub fn server_name_for_file(&self, file: &str) -> Option<&str> {
        let ext = Path::new(file)
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))?;
        self.config.server_for_extension(&ext)
    }

    /// Get or lazily start the LSP server for a given file.
    pub async fn get_or_start(&mut self, file: &str) -> crate::Result<&mut LspStdioClient> {
        let server_name = self.server_name_for_file(file)
            .ok_or_else(|| Error::Lsp {
                server: String::new(),
                message: format!(
                    "No LSP server configured for '{}'. Add one to .viv/settings.json under lspServers.",
                    file
                ),
            })?
            .to_string();

        if self.servers.get(&server_name).and_then(|s| s.as_ref()).is_none() {
            self.start_server(&server_name).await?;
        }

        self.servers
            .get_mut(&server_name)
            .and_then(|s| s.as_mut())
            .ok_or_else(|| Error::Lsp {
                server: server_name,
                message: "server failed to start".into(),
            })
    }

    async fn start_server(&mut self, name: &str) -> crate::Result<()> {
        let server_config = self.config.servers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, c)| c)
            .ok_or_else(|| Error::Lsp {
                server: name.into(),
                message: format!("no config for server '{}'", name),
            })?;

        let env_map: HashMap<String, String> = server_config.env.iter().cloned().collect();
        let args_refs: Vec<&str> = server_config.args.iter().map(|s| s.as_str()).collect();

        let transport = StdioTransport::spawn_with_framing(
            &server_config.command,
            &args_refs,
            &env_map,
            Framing::ContentLength,
        )?;

        let mut client = LspClient::new(transport, name);
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());
        client.initialize(&cwd).await?;

        self.servers.insert(name.to_string(), Some(client));
        Ok(())
    }

    /// Ensure a file is opened with the LSP server (sends didOpen if not yet).
    pub async fn ensure_file_open(&mut self, file: &str) -> crate::Result<()> {
        if self.opened_files.contains(file) {
            return Ok(());
        }
        let uri = path_to_uri(file);
        let content = std::fs::read_to_string(file).map_err(crate::Error::Io)?;
        let lang_id = language_id_from_path(file);

        let client = self.get_or_start(file).await?;
        client.did_open(&uri, lang_id, &content).await?;
        self.opened_files.insert(file.to_string());
        Ok(())
    }

    pub async fn shutdown_all(&mut self) {
        for (_, client_opt) in &mut self.servers {
            if let Some(client) = client_opt.take() {
                let mut client = client;
                let _ = client.shutdown().await;
            }
        }
    }
}

pub fn path_to_uri(path: &str) -> String {
    let abs = if Path::new(path).is_absolute() {
        path.to_string()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path).to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string())
    };
    format!("file://{}", abs)
}

fn language_id_from_path(path: &str) -> &str {
    match Path::new(path).extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("py") => "python",
        Some("js") => "javascript",
        Some("ts") => "typescript",
        Some("go") => "go",
        Some("c") | Some("h") => "c",
        Some("cpp") | Some("hpp") | Some("cc") => "cpp",
        Some("java") => "java",
        Some("rb") => "ruby",
        _ => "plaintext",
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test lsp_tests manager_test`
Expected: all 2 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lsp/mod.rs tests/lsp/manager_test.rs tests/lsp/mod.rs
git commit -m "feat(lsp): add LspManager with lazy server lifecycle"
```

---

### Task 7: LSP Tools

**Files:**
- Create: `src/lsp/tools.rs`

- [ ] **Step 1: Write failing tests for LSP tools**

Create `tests/lsp/tools_test.rs`:

```rust
use viv::lsp::tools::{LspDefinitionTool, LspReferencesTool, LspHoverTool, LspDiagnosticsTool};
use viv::tools::Tool;

#[test]
fn definition_tool_metadata() {
    let mgr = std::sync::Arc::new(std::sync::Mutex::new(empty_manager()));
    let tool = LspDefinitionTool::new(mgr);
    assert_eq!(tool.name(), "LspDefinition");
    assert_eq!(tool.permission_level(), viv::tools::PermissionLevel::ReadOnly);

    let schema = tool.input_schema();
    let props = schema.get("properties").unwrap();
    assert!(props.get("file").is_some());
    assert!(props.get("line").is_some());
    assert!(props.get("column").is_some());
}

#[test]
fn references_tool_metadata() {
    let mgr = std::sync::Arc::new(std::sync::Mutex::new(empty_manager()));
    let tool = LspReferencesTool::new(mgr);
    assert_eq!(tool.name(), "LspReferences");
}

#[test]
fn hover_tool_metadata() {
    let mgr = std::sync::Arc::new(std::sync::Mutex::new(empty_manager()));
    let tool = LspHoverTool::new(mgr);
    assert_eq!(tool.name(), "LspHover");
}

#[test]
fn diagnostics_tool_metadata() {
    let mgr = std::sync::Arc::new(std::sync::Mutex::new(empty_manager()));
    let tool = LspDiagnosticsTool::new(mgr);
    assert_eq!(tool.name(), "LspDiagnostics");

    let schema = tool.input_schema();
    let required = schema.get("required");
    // file is optional for diagnostics
    assert!(required.is_none() || required.unwrap().as_array().map_or(true, |a| a.is_empty()));
}

fn empty_manager() -> viv::lsp::LspManager {
    let config = viv::lsp::config::LspConfig::parse("{}").unwrap();
    viv::lsp::LspManager::new(config)
}
```

Add `pub mod tools_test;` to `tests/lsp/mod.rs`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test lsp_tests tools_test`
Expected: compilation error — `viv::lsp::tools` does not exist.

- [ ] **Step 3: Implement LSP tools**

Create `src/lsp/tools.rs`:

```rust
#![allow(clippy::await_holding_lock)]

use super::LspManager;
use super::path_to_uri;
use crate::core::json::JsonValue;
use crate::core::runtime::AssertSend;
use crate::tools::{PermissionLevel, Tool};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

fn file_line_col_schema() -> JsonValue {
    JsonValue::Object(vec![
        ("type".into(), JsonValue::Str("object".into())),
        ("properties".into(), JsonValue::Object(vec![
            ("file".into(), JsonValue::Object(vec![
                ("type".into(), JsonValue::Str("string".into())),
                ("description".into(), JsonValue::Str("File path (relative or absolute)".into())),
            ])),
            ("line".into(), JsonValue::Object(vec![
                ("type".into(), JsonValue::Str("integer".into())),
                ("description".into(), JsonValue::Str("Line number (0-based)".into())),
            ])),
            ("column".into(), JsonValue::Object(vec![
                ("type".into(), JsonValue::Str("integer".into())),
                ("description".into(), JsonValue::Str("Column number (0-based)".into())),
            ])),
        ])),
        ("required".into(), JsonValue::Array(vec![
            JsonValue::Str("file".into()),
            JsonValue::Str("line".into()),
            JsonValue::Str("column".into()),
        ])),
    ])
}

fn extract_file_line_col(input: &JsonValue) -> (String, u32, u32) {
    let file = input.get("file").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let line = input.get("line").and_then(|v| v.as_i64()).unwrap_or(0) as u32;
    let col = input.get("column").and_then(|v| v.as_i64()).unwrap_or(0) as u32;
    (file, line, col)
}

/// Read a few lines around a given line number from a file for context.
fn read_context_lines(file: &str, line: u32, context: u32) -> String {
    let Ok(content) = std::fs::read_to_string(file) else { return String::new() };
    let lines: Vec<&str> = content.lines().collect();
    let start = line.saturating_sub(context) as usize;
    let end = ((line + context + 1) as usize).min(lines.len());
    lines[start..end]
        .iter()
        .enumerate()
        .map(|(i, l)| format!("{:>4} {}", start + i + 1, l))
        .collect::<Vec<_>>()
        .join("\n")
}

// ── LspDefinitionTool ────────────────────────────────────────────────────────

pub struct LspDefinitionTool {
    manager: Arc<Mutex<LspManager>>,
}

impl LspDefinitionTool {
    pub fn new(manager: Arc<Mutex<LspManager>>) -> Self {
        LspDefinitionTool { manager }
    }
}

impl Tool for LspDefinitionTool {
    fn name(&self) -> &str { "LspDefinition" }

    fn description(&self) -> &str {
        "Go to the definition of a symbol at the given file position. Returns the file path, line number, and surrounding code of the definition."
    }

    fn input_schema(&self) -> JsonValue { file_line_col_schema() }

    fn execute(&self, input: &JsonValue) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let (file, line, col) = extract_file_line_col(input);
        Box::pin(AssertSend(async move {
            let mut mgr = self.manager.lock().unwrap();
            mgr.ensure_file_open(&file).await?;
            let uri = path_to_uri(&file);
            let client = mgr.get_or_start(&file).await?;
            let locations = client.definition(&uri, line, col).await?;

            if locations.is_empty() {
                return Ok("No definition found.".to_string());
            }

            let mut output = String::new();
            for loc in &locations {
                let path = loc.file_path().unwrap_or(&loc.uri);
                let l = loc.range.start.line;
                let c = loc.range.start.character;
                output.push_str(&format!("{}:{}:{}\n", path, l + 1, c + 1));
                let context = read_context_lines(path, l, 2);
                if !context.is_empty() {
                    output.push_str(&context);
                    output.push('\n');
                }
            }
            Ok(output)
        }))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }
}

// ── LspReferencesTool ────────────────────────────────────────────────────────

pub struct LspReferencesTool {
    manager: Arc<Mutex<LspManager>>,
}

impl LspReferencesTool {
    pub fn new(manager: Arc<Mutex<LspManager>>) -> Self {
        LspReferencesTool { manager }
    }
}

impl Tool for LspReferencesTool {
    fn name(&self) -> &str { "LspReferences" }

    fn description(&self) -> &str {
        "Find all references to a symbol at the given file position. Returns a list of locations where the symbol is used."
    }

    fn input_schema(&self) -> JsonValue { file_line_col_schema() }

    fn execute(&self, input: &JsonValue) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let (file, line, col) = extract_file_line_col(input);
        Box::pin(AssertSend(async move {
            let mut mgr = self.manager.lock().unwrap();
            mgr.ensure_file_open(&file).await?;
            let uri = path_to_uri(&file);
            let client = mgr.get_or_start(&file).await?;
            let locations = client.references(&uri, line, col).await?;

            if locations.is_empty() {
                return Ok("No references found.".to_string());
            }

            let total = locations.len();
            let mut output = format!("Found {} reference(s):\n", total);
            for loc in locations.iter().take(20) {
                let path = loc.file_path().unwrap_or(&loc.uri);
                let l = loc.range.start.line;
                let c = loc.range.start.character;
                // Read just the referenced line
                let line_text = read_single_line(path, l);
                output.push_str(&format!("  {}:{}:{}", path, l + 1, c + 1));
                if !line_text.is_empty() {
                    output.push_str(&format!(" — {}", line_text.trim()));
                }
                output.push('\n');
            }
            if total > 20 {
                output.push_str(&format!("  ... and {} more\n", total - 20));
            }
            Ok(output)
        }))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }
}

// ── LspHoverTool ─────────────────────────────────────────────────────────────

pub struct LspHoverTool {
    manager: Arc<Mutex<LspManager>>,
}

impl LspHoverTool {
    pub fn new(manager: Arc<Mutex<LspManager>>) -> Self {
        LspHoverTool { manager }
    }
}

impl Tool for LspHoverTool {
    fn name(&self) -> &str { "LspHover" }

    fn description(&self) -> &str {
        "Get type information and documentation for a symbol at the given file position."
    }

    fn input_schema(&self) -> JsonValue { file_line_col_schema() }

    fn execute(&self, input: &JsonValue) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let (file, line, col) = extract_file_line_col(input);
        Box::pin(AssertSend(async move {
            let mut mgr = self.manager.lock().unwrap();
            mgr.ensure_file_open(&file).await?;
            let uri = path_to_uri(&file);
            let client = mgr.get_or_start(&file).await?;
            let hover = client.hover(&uri, line, col).await?;

            match hover {
                Some(h) => Ok(h.contents),
                None => Ok("No hover information available.".to_string()),
            }
        }))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }
}

// ── LspDiagnosticsTool ──────────────────────────────────────────────────────

pub struct LspDiagnosticsTool {
    manager: Arc<Mutex<LspManager>>,
}

impl LspDiagnosticsTool {
    pub fn new(manager: Arc<Mutex<LspManager>>) -> Self {
        LspDiagnosticsTool { manager }
    }
}

impl Tool for LspDiagnosticsTool {
    fn name(&self) -> &str { "LspDiagnostics" }

    fn description(&self) -> &str {
        "Get diagnostics (errors, warnings) from the LSP server. Optionally filter by file path."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("type".into(), JsonValue::Str("object".into())),
            ("properties".into(), JsonValue::Object(vec![
                ("file".into(), JsonValue::Object(vec![
                    ("type".into(), JsonValue::Str("string".into())),
                    ("description".into(), JsonValue::Str(
                        "Optional file path to filter diagnostics. If omitted, returns all diagnostics.".into()
                    )),
                ])),
            ])),
        ])
    }

    fn execute(&self, input: &JsonValue) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let file = input.get("file").and_then(|v| v.as_str()).map(|s| s.to_string());
        Box::pin(AssertSend(async move {
            let mgr = self.manager.lock().unwrap();
            let mut output = String::new();

            match file {
                Some(f) => {
                    let uri = path_to_uri(&f);
                    // Check all servers for diagnostics matching this URI
                    for (_, client_opt) in &mgr.servers {
                        if let Some(client) = client_opt {
                            let diags = client.diagnostics(&uri);
                            format_diagnostics(&mut output, &f, &diags);
                        }
                    }
                }
                None => {
                    for (_, client_opt) in &mgr.servers {
                        if let Some(client) = client_opt {
                            for (uri, diags) in client.all_diagnostics() {
                                let path = uri.strip_prefix("file://").unwrap_or(uri);
                                format_diagnostics(&mut output, path, diags);
                            }
                        }
                    }
                }
            }

            if output.is_empty() {
                output.push_str("No diagnostics.");
            }
            Ok(output)
        }))
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }
}

fn format_diagnostics(output: &mut String, path: &str, diags: &[super::types::Diagnostic]) {
    if diags.is_empty() {
        return;
    }
    output.push_str(&format!("{}:\n", path));
    for d in diags {
        let l = d.range.start.line + 1;
        let c = d.range.start.character + 1;
        let code_str = d.code.as_deref().map(|c| format!("[{}] ", c)).unwrap_or_default();
        output.push_str(&format!("  {}:{} {} {}{}\n", l, c, d.severity.label(), code_str, d.message));
    }
}

fn read_single_line(file: &str, line: u32) -> String {
    let Ok(content) = std::fs::read_to_string(file) else { return String::new() };
    content.lines().nth(line as usize).unwrap_or("").to_string()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test lsp_tests tools_test`
Expected: all 4 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/lsp/tools.rs tests/lsp/tools_test.rs tests/lsp/mod.rs
git commit -m "feat(lsp): add 4 LSP tools (Definition, References, Hover, Diagnostics)"
```

---

### Task 8: Agent integration

**Files:**
- Modify: `src/agent/agent.rs`

- [ ] **Step 1: Import LSP modules in agent**

Add to imports in `src/agent/agent.rs`:

```rust
use crate::lsp::LspManager;
use crate::lsp::config::LspConfig;
use crate::lsp::tools::{LspDefinitionTool, LspReferencesTool, LspHoverTool, LspDiagnosticsTool};
```

- [ ] **Step 2: Add lsp field to Agent struct**

Add `lsp: Arc<Mutex<LspManager>>,` to the `Agent` struct.

- [ ] **Step 3: Initialize LspManager in Agent::new()**

After the MCP registration block, add:

```rust
// Load LSP config (reuse same settings.json)
let lsp_config = LspConfig::load(".viv/settings.json")?;
let lsp = Arc::new(Mutex::new(LspManager::new(lsp_config)));

// Register LSP tools
tools.register(Box::new(LspDefinitionTool::new(lsp.clone())));
tools.register(Box::new(LspReferencesTool::new(lsp.clone())));
tools.register(Box::new(LspHoverTool::new(lsp.clone())));
tools.register(Box::new(LspDiagnosticsTool::new(lsp.clone())));
```

Add `lsp` to the `Agent` struct initialization.

- [ ] **Step 4: Add LSP shutdown to Agent::shutdown()**

In `shutdown()`, after the MCP shutdown block, add:

```rust
// Shutdown LSP servers
{
    let mut lsp_mgr = self.lsp.lock().unwrap();
    lsp_mgr.shutdown_all().await;
}
```

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: all tests PASS.

Run: `cargo clippy`
Expected: no new warnings.

- [ ] **Step 6: Commit**

```bash
git add src/agent/agent.rs
git commit -m "feat(agent): integrate LSP tools into Agent lifecycle"
```

---

### Task 9: Full integration test

**Files:**
- Create: `tests/lsp/integration_test.rs`
- Modify: `tests/lsp/mod.rs`

- [ ] **Step 1: Write integration test with end-to-end flow**

Create `tests/lsp/integration_test.rs`:

```rust
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use viv::core::json::{JsonValue, Number};
use viv::core::runtime::block_on;
use viv::lsp::LspManager;
use viv::lsp::config::LspConfig;
use viv::lsp::tools::{LspDefinitionTool, LspHoverTool};
use viv::tools::Tool;

/// Verify that tools return helpful message when no LSP server is configured.
#[test]
fn tool_returns_error_when_no_server_configured() {
    let config = LspConfig::parse("{}").unwrap();
    let mgr = Arc::new(Mutex::new(LspManager::new(config)));
    let tool = LspDefinitionTool::new(mgr);

    let input = JsonValue::Object(vec![
        ("file".into(), JsonValue::Str("src/main.rs".into())),
        ("line".into(), JsonValue::Number(Number::Int(0))),
        ("column".into(), JsonValue::Number(Number::Int(0))),
    ]);

    let result = block_on(async move { tool.execute(&input).await });
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("No LSP server configured"), "got: {}", err_msg);
}
```

Add `pub mod integration_test;` to `tests/lsp/mod.rs`.

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --test lsp_tests integration_test`
Expected: PASS.

- [ ] **Step 3: Run full test suite and clippy**

Run: `cargo test && cargo clippy`
Expected: all tests PASS, no clippy warnings.

- [ ] **Step 4: Commit**

```bash
git add tests/lsp/integration_test.rs tests/lsp/mod.rs
git commit -m "test(lsp): add integration tests for LSP tools"
```
