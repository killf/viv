// MutexGuard is intentionally held across await points in this module.
// All futures here run inside block_on_local (single-threaded), so the
// MutexGuard is never actually sent between threads.
#![allow(clippy::await_holding_lock)]

use super::McpManager;
use crate::core::json::JsonValue;
use crate::core::runtime::AssertSend;
use crate::core::sync::lock_or_recover;
use crate::tools::{PermissionLevel, Tool};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

// ── McpToolProxy ─────────────────────────────────────────────────────────────

/// Proxy that exposes an MCP server tool as a local Tool.
pub struct McpToolProxy {
    full_name: String, // "mcp__serverName__toolName"
    server_name: String,
    tool_name: String,
    description: String,
    schema: JsonValue,
    manager: Arc<Mutex<McpManager>>,
}

impl McpToolProxy {
    pub fn new(
        server_name: &str,
        tool_name: &str,
        description: &str,
        schema: JsonValue,
        manager: Arc<Mutex<McpManager>>,
    ) -> Self {
        McpToolProxy {
            full_name: format!("mcp__{}__{}", server_name, tool_name),
            server_name: server_name.to_string(),
            tool_name: tool_name.to_string(),
            description: description.to_string(),
            schema,
            manager,
        }
    }
}

impl Tool for McpToolProxy {
    fn name(&self) -> &str {
        &self.full_name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn input_schema(&self) -> JsonValue {
        self.schema.clone()
    }

    fn execute(
        &self,
        input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let input = input.clone();
        Box::pin(AssertSend(async move {
            let mut mgr = lock_or_recover(&self.manager);
            mgr.call_tool(&self.server_name, &self.tool_name, &input)
                .await
        }))
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Execute
    }
}

// ── ListMcpResourcesTool ─────────────────────────────────────────────────────

/// Lists all resources across all connected MCP servers.
pub struct ListMcpResourcesTool {
    manager: Arc<Mutex<McpManager>>,
}

impl ListMcpResourcesTool {
    pub fn new(manager: Arc<Mutex<McpManager>>) -> Self {
        ListMcpResourcesTool { manager }
    }
}

impl Tool for ListMcpResourcesTool {
    fn name(&self) -> &str {
        "mcp__list_resources"
    }

    fn description(&self) -> &str {
        "List all available MCP resources across all connected servers"
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("type".to_string(), JsonValue::Str("object".to_string())),
            ("properties".to_string(), JsonValue::Object(vec![])),
        ])
    }

    fn execute(
        &self,
        _input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        Box::pin(async move {
            let mgr = lock_or_recover(&self.manager);
            let mut output = String::new();
            for handle in &mgr.servers {
                for res in &handle.resources {
                    output.push_str(&format!("[{}] {} ({})", handle.name, res.name, res.uri,));
                    if let Some(desc) = &res.description {
                        output.push_str(&format!(" - {}", desc));
                    }
                    output.push('\n');
                }
            }
            if output.is_empty() {
                output.push_str("No MCP resources available.");
            }
            Ok(output)
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
}

// ── ReadMcpResourceTool ──────────────────────────────────────────────────────

/// Reads a specific resource from an MCP server.
pub struct ReadMcpResourceTool {
    manager: Arc<Mutex<McpManager>>,
}

impl ReadMcpResourceTool {
    pub fn new(manager: Arc<Mutex<McpManager>>) -> Self {
        ReadMcpResourceTool { manager }
    }
}

impl Tool for ReadMcpResourceTool {
    fn name(&self) -> &str {
        "mcp__read_resource"
    }

    fn description(&self) -> &str {
        "Read a resource from an MCP server by server name and URI"
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("type".to_string(), JsonValue::Str("object".to_string())),
            (
                "properties".to_string(),
                JsonValue::Object(vec![
                    (
                        "server".to_string(),
                        JsonValue::Object(vec![
                            ("type".to_string(), JsonValue::Str("string".to_string())),
                            (
                                "description".to_string(),
                                JsonValue::Str("MCP server name".to_string()),
                            ),
                        ]),
                    ),
                    (
                        "uri".to_string(),
                        JsonValue::Object(vec![
                            ("type".to_string(), JsonValue::Str("string".to_string())),
                            (
                                "description".to_string(),
                                JsonValue::Str("Resource URI".to_string()),
                            ),
                        ]),
                    ),
                ]),
            ),
            (
                "required".to_string(),
                JsonValue::Array(vec![
                    JsonValue::Str("server".to_string()),
                    JsonValue::Str("uri".to_string()),
                ]),
            ),
        ])
    }

    fn execute(
        &self,
        input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let server = input
            .get("server")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let uri = input
            .get("uri")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Box::pin(AssertSend(async move {
            let mut mgr = lock_or_recover(&self.manager);
            mgr.read_resource(&server, &uri).await
        }))
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
}

// ── ListMcpPromptsTool ───────────────────────────────────────────────────────

/// Lists all prompts across all connected MCP servers.
pub struct ListMcpPromptsTool {
    manager: Arc<Mutex<McpManager>>,
}

impl ListMcpPromptsTool {
    pub fn new(manager: Arc<Mutex<McpManager>>) -> Self {
        ListMcpPromptsTool { manager }
    }
}

impl Tool for ListMcpPromptsTool {
    fn name(&self) -> &str {
        "mcp__list_prompts"
    }

    fn description(&self) -> &str {
        "List all available MCP prompts across all connected servers"
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("type".to_string(), JsonValue::Str("object".to_string())),
            ("properties".to_string(), JsonValue::Object(vec![])),
        ])
    }

    fn execute(
        &self,
        _input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        Box::pin(async move {
            let mgr = lock_or_recover(&self.manager);
            let mut output = String::new();
            for handle in &mgr.servers {
                for prompt in &handle.prompts {
                    output.push_str(&format!("[{}] {}", handle.name, prompt.name));
                    if let Some(desc) = &prompt.description {
                        output.push_str(&format!(" - {}", desc));
                    }
                    if !prompt.arguments.is_empty() {
                        let arg_names: Vec<&str> =
                            prompt.arguments.iter().map(|a| a.name.as_str()).collect();
                        output.push_str(&format!(" (args: {})", arg_names.join(", ")));
                    }
                    output.push('\n');
                }
            }
            if output.is_empty() {
                output.push_str("No MCP prompts available.");
            }
            Ok(output)
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
}

// ── GetMcpPromptTool ─────────────────────────────────────────────────────────

/// Gets a specific prompt from an MCP server.
pub struct GetMcpPromptTool {
    manager: Arc<Mutex<McpManager>>,
}

impl GetMcpPromptTool {
    pub fn new(manager: Arc<Mutex<McpManager>>) -> Self {
        GetMcpPromptTool { manager }
    }
}

impl Tool for GetMcpPromptTool {
    fn name(&self) -> &str {
        "mcp__get_prompt"
    }

    fn description(&self) -> &str {
        "Get a prompt from an MCP server by server name and prompt name"
    }

    fn input_schema(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("type".to_string(), JsonValue::Str("object".to_string())),
            (
                "properties".to_string(),
                JsonValue::Object(vec![
                    (
                        "server".to_string(),
                        JsonValue::Object(vec![
                            ("type".to_string(), JsonValue::Str("string".to_string())),
                            (
                                "description".to_string(),
                                JsonValue::Str("MCP server name".to_string()),
                            ),
                        ]),
                    ),
                    (
                        "name".to_string(),
                        JsonValue::Object(vec![
                            ("type".to_string(), JsonValue::Str("string".to_string())),
                            (
                                "description".to_string(),
                                JsonValue::Str("Prompt name".to_string()),
                            ),
                        ]),
                    ),
                    (
                        "arguments".to_string(),
                        JsonValue::Object(vec![
                            ("type".to_string(), JsonValue::Str("object".to_string())),
                            (
                                "description".to_string(),
                                JsonValue::Str("Prompt arguments as key-value pairs".to_string()),
                            ),
                        ]),
                    ),
                ]),
            ),
            (
                "required".to_string(),
                JsonValue::Array(vec![
                    JsonValue::Str("server".to_string()),
                    JsonValue::Str("name".to_string()),
                ]),
            ),
        ])
    }

    fn execute(
        &self,
        input: &JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<String>> + Send + '_>> {
        let server = input
            .get("server")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let args = input
            .get("arguments")
            .cloned()
            .unwrap_or(JsonValue::Object(vec![]));

        Box::pin(AssertSend(async move {
            let mut mgr = lock_or_recover(&self.manager);
            mgr.get_prompt(&server, &name, &args).await
        }))
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
}
