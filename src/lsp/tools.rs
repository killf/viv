// MutexGuard is intentionally held across await points in this module.
// All futures here run inside block_on_local (single-threaded), so the
// MutexGuard is never actually sent between threads.
#![allow(clippy::await_holding_lock)]

use super::LspManager;
#[cfg(unix)]
use super::path_to_uri;
use crate::core::json::JsonValue;
use crate::core::runtime::AssertSend;
use crate::core::sync::lock_or_recover;
use crate::lsp::types::Diagnostic;
use crate::tools::{PermissionLevel, Tool};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Returns a platform-unsupported error on non-unix platforms.
#[cfg(not(unix))]
fn platform_unsupported() -> crate::Result<String> {
    Err(crate::Error::Lsp {
        server: "<none>".to_string(),
        message: "LSP over stdio is not yet supported on this platform".to_string(),
    })
}

/// Shared input schema for tools that take file, line, and column.
fn file_line_col_schema() -> JsonValue {
    JsonValue::Object(vec![
        ("type".to_string(), JsonValue::Str("object".to_string())),
        (
            "properties".to_string(),
            JsonValue::Object(vec![
                (
                    "file".to_string(),
                    JsonValue::Object(vec![
                        ("type".to_string(), JsonValue::Str("string".to_string())),
                        (
                            "description".to_string(),
                            JsonValue::Str("Absolute path to the file".to_string()),
                        ),
                    ]),
                ),
                (
                    "line".to_string(),
                    JsonValue::Object(vec![
                        ("type".to_string(), JsonValue::Str("integer".to_string())),
                        (
                            "description".to_string(),
                            JsonValue::Str("0-based line number".to_string()),
                        ),
                    ]),
                ),
                (
                    "column".to_string(),
                    JsonValue::Object(vec![
                        ("type".to_string(), JsonValue::Str("integer".to_string())),
                        (
                            "description".to_string(),
                            JsonValue::Str("0-based column number".to_string()),
                        ),
                    ]),
                ),
            ]),
        ),
        (
            "required".to_string(),
            JsonValue::Array(vec![
                JsonValue::Str("file".to_string()),
                JsonValue::Str("line".to_string()),
                JsonValue::Str("column".to_string()),
            ]),
        ),
    ])
}

/// Extract file, line, and column from tool input JSON.
fn extract_file_line_col(input: &JsonValue) -> (String, u32, u32) {
    let file = input
        .get("file")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let line = input.get("line").and_then(|v| v.as_i64()).unwrap_or(0) as u32;
    let col = input.get("column").and_then(|v| v.as_i64()).unwrap_or(0) as u32;
    (file, line, col)
}

/// Read N lines before and after the given line from a file on disk.
#[cfg(unix)]
fn read_context_lines(file: &str, line: u32, context: u32) -> String {
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len() as u32;
    if total == 0 {
        return String::new();
    }

    let start = line.saturating_sub(context);
    let end = (line + context + 1).min(total);

    let mut result = String::new();
    for i in start..end {
        result.push_str(&format!("{}: {}\n", i, lines[i as usize]));
    }
    result
}

/// Read a single line from a file on disk.
#[cfg(unix)]
fn read_single_line(file: &str, line: u32) -> String {
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    content.lines().nth(line as usize).unwrap_or("").to_string()
}

/// Format diagnostics for a given path into the output string.
fn format_diagnostics(output: &mut String, path: &str, diags: &[Diagnostic]) {
    for diag in diags {
        let severity = diag
            .severity
            .as_ref()
            .map(|s| s.label())
            .unwrap_or("unknown");
        let line = diag.range.start.line;
        let col = diag.range.start.character;
        let code = diag
            .code
            .as_deref()
            .map(|c| format!(" [{}]", c))
            .unwrap_or_default();
        output.push_str(&format!(
            "{}:{}:{}: {}{}: {}\n",
            path, line, col, severity, code, diag.message
        ));
    }
}

// ---------------------------------------------------------------------------
// LspDefinitionTool
// ---------------------------------------------------------------------------

/// Finds the definition of a symbol at the given file position.
pub struct LspDefinitionTool {
    manager: Arc<Mutex<LspManager>>,
}

impl LspDefinitionTool {
    pub fn new(manager: Arc<Mutex<LspManager>>) -> Self {
        LspDefinitionTool { manager }
    }
}

impl Tool for LspDefinitionTool {
    fn name(&self) -> &str {
        "LspDefinition"
    }

    fn description(&self) -> &str {
        "Go to the definition of a symbol at the given file position. Returns the file path, line number, and surrounding code of the definition."
    }

    fn input_schema(&self) -> JsonValue {
        file_line_col_schema()
    }

    fn execute(
        &self,
        input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let (file, line, col) = extract_file_line_col(input);
        Box::pin(AssertSend(async move {
            #[cfg(not(unix))]
            {
                let _ = (&file, line, col);
                return platform_unsupported();
            }

            #[cfg(unix)]
            {
                let mut mgr = lock_or_recover(&self.manager);
                mgr.ensure_file_open(&file).await?;
                let uri = path_to_uri(&file);
                let client = mgr.get_or_start(&file).await?;
                let locations = client.definition(&uri, line, col).await?;

                if locations.is_empty() {
                    return Ok("No definition found.".to_string());
                }

                let mut output = String::new();
                for loc in &locations {
                    let path = loc.file_path();
                    let def_line = loc.range.start.line;
                    let def_col = loc.range.start.character;
                    output.push_str(&format!("{}:{}:{}\n", path, def_line, def_col));
                    let context = read_context_lines(path, def_line, 2);
                    if !context.is_empty() {
                        output.push_str(&context);
                    }
                }
                Ok(output)
            }
        }))
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
}

// ---------------------------------------------------------------------------
// LspReferencesTool
// ---------------------------------------------------------------------------

/// Finds all references to a symbol at the given file position.
pub struct LspReferencesTool {
    manager: Arc<Mutex<LspManager>>,
}

impl LspReferencesTool {
    pub fn new(manager: Arc<Mutex<LspManager>>) -> Self {
        LspReferencesTool { manager }
    }
}

impl Tool for LspReferencesTool {
    fn name(&self) -> &str {
        "LspReferences"
    }

    fn description(&self) -> &str {
        "Find all references to a symbol at the given file position. Returns a list of locations where the symbol is used."
    }

    fn input_schema(&self) -> JsonValue {
        file_line_col_schema()
    }

    fn execute(
        &self,
        input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let (file, line, col) = extract_file_line_col(input);
        Box::pin(AssertSend(async move {
            #[cfg(not(unix))]
            {
                let _ = (&file, line, col);
                return platform_unsupported();
            }

            #[cfg(unix)]
            {
                let mut mgr = lock_or_recover(&self.manager);
                mgr.ensure_file_open(&file).await?;
                let uri = path_to_uri(&file);
                let client = mgr.get_or_start(&file).await?;
                let locations = client.references(&uri, line, col).await?;

                if locations.is_empty() {
                    return Ok("No references found.".to_string());
                }

                let total = locations.len();
                let truncated = locations.len() > 20;
                let shown = locations.iter().take(20);

                let mut output = format!("Found {} reference(s):\n", total);
                for loc in shown {
                    let path = loc.file_path();
                    let ref_line = loc.range.start.line;
                    let ref_col = loc.range.start.character;
                    let code = read_single_line(path, ref_line);
                    if code.is_empty() {
                        output.push_str(&format!("{}:{}:{}\n", path, ref_line, ref_col));
                    } else {
                        output.push_str(&format!(
                            "{}:{}:{}: {}\n",
                            path,
                            ref_line,
                            ref_col,
                            code.trim()
                        ));
                    }
                }
                if truncated {
                    output.push_str(&format!("... ({} more not shown)\n", total - 20));
                }
                Ok(output)
            }
        }))
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
}

// ---------------------------------------------------------------------------
// LspHoverTool
// ---------------------------------------------------------------------------

/// Gets type information and documentation for a symbol.
pub struct LspHoverTool {
    manager: Arc<Mutex<LspManager>>,
}

impl LspHoverTool {
    pub fn new(manager: Arc<Mutex<LspManager>>) -> Self {
        LspHoverTool { manager }
    }
}

impl Tool for LspHoverTool {
    fn name(&self) -> &str {
        "LspHover"
    }

    fn description(&self) -> &str {
        "Get type information and documentation for a symbol at the given file position."
    }

    fn input_schema(&self) -> JsonValue {
        file_line_col_schema()
    }

    fn execute(
        &self,
        input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let (file, line, col) = extract_file_line_col(input);
        Box::pin(AssertSend(async move {
            #[cfg(not(unix))]
            {
                let _ = (&file, line, col);
                return platform_unsupported();
            }

            #[cfg(unix)]
            {
                let mut mgr = lock_or_recover(&self.manager);
                mgr.ensure_file_open(&file).await?;
                let uri = path_to_uri(&file);
                let client = mgr.get_or_start(&file).await?;
                let result = client.hover(&uri, line, col).await?;

                match result {
                    Some(hover) => Ok(hover.contents),
                    None => Ok("No hover information available.".to_string()),
                }
            }
        }))
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
}

// ---------------------------------------------------------------------------
// LspDiagnosticsTool
// ---------------------------------------------------------------------------

/// Gets diagnostics (errors, warnings) from the LSP server.
pub struct LspDiagnosticsTool {
    manager: Arc<Mutex<LspManager>>,
}

impl LspDiagnosticsTool {
    pub fn new(manager: Arc<Mutex<LspManager>>) -> Self {
        LspDiagnosticsTool { manager }
    }
}

impl Tool for LspDiagnosticsTool {
    fn name(&self) -> &str {
        "LspDiagnostics"
    }

    fn description(&self) -> &str {
        "Get diagnostics (errors, warnings) from the LSP server. Optionally filter by file path."
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("type".to_string(), JsonValue::Str("object".to_string())),
            (
                "properties".to_string(),
                JsonValue::Object(vec![(
                    "file".to_string(),
                    JsonValue::Object(vec![
                        ("type".to_string(), JsonValue::Str("string".to_string())),
                        (
                            "description".to_string(),
                            JsonValue::Str("Optional file path to filter diagnostics".to_string()),
                        ),
                    ]),
                )]),
            ),
        ])
    }

    fn execute(
        &self,
        input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let file = input
            .get("file")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        Box::pin(AssertSend(async move {
            #[cfg(not(unix))]
            {
                let _ = &file;
                return platform_unsupported();
            }

            #[cfg(unix)]
            {
                let mut mgr = lock_or_recover(&self.manager);
                let mut output = String::new();

                if let Some(ref path) = file {
                    // Ensure the file is open so diagnostics are available.
                    mgr.ensure_file_open(path).await?;
                    let uri = path_to_uri(path);
                    let client = mgr.get_or_start(path).await?;
                    let diags = client.diagnostics(&uri);
                    format_diagnostics(&mut output, path, &diags);
                } else {
                    // Collect all diagnostics across all running servers.
                    for (_name, slot) in mgr.servers.iter() {
                        if let Some(client) = slot.as_ref() {
                            for (uri, diags) in client.all_diagnostics() {
                                let path = uri.strip_prefix("file://").unwrap_or(uri);
                                format_diagnostics(&mut output, path, diags);
                            }
                        }
                    }
                }

                if output.is_empty() {
                    output.push_str("No diagnostics.");
                }
                Ok(output)
            }
        }))
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
}
