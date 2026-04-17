pub mod client;
pub mod config;
pub mod tools;
pub mod transport;
pub mod types;

use crate::Error;
use crate::core::json::JsonValue;
use client::McpClient;
use config::{McpConfig, ServerConfig};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use transport::stdio::StdioTransport;
use types::*;

// ── McpClientKind ────────────────────────────────────────────────────────────

enum McpClientKind {
    Stdio(McpClient<StdioTransport>),
}

impl McpClientKind {
    fn call_tool<'a>(
        &'a mut self,
        name: &'a str,
        args: &'a JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<ToolCallResult>> + Send + 'a>> {
        match self {
            McpClientKind::Stdio(c) => Box::pin(c.call_tool(name, args)),
        }
    }

    fn read_resource<'a>(
        &'a mut self,
        uri: &'a str,
    ) -> Pin<Box<dyn Future<Output = crate::Result<ResourceContent>> + Send + 'a>> {
        match self {
            McpClientKind::Stdio(c) => Box::pin(c.read_resource(uri)),
        }
    }

    fn get_prompt<'a>(
        &'a mut self,
        name: &'a str,
        args: &'a JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<PromptMessages>> + Send + 'a>> {
        match self {
            McpClientKind::Stdio(c) => Box::pin(c.get_prompt(name, args)),
        }
    }

    fn close(&mut self) -> Pin<Box<dyn Future<Output = crate::Result<()>> + Send + '_>> {
        match self {
            McpClientKind::Stdio(c) => Box::pin(c.shutdown()),
        }
    }
}

// ── McpServerHandle ──────────────────────────────────────────────────────────

pub struct McpServerHandle {
    pub name: String,
    pub tools: Vec<McpTool>,
    pub resources: Vec<McpResource>,
    pub prompts: Vec<McpPrompt>,
    client: McpClientKind,
}

// ── McpManager ───────────────────────────────────────────────────────────────

pub struct McpManager {
    pub servers: Vec<McpServerHandle>,
}

impl McpManager {
    /// Connect to all MCP servers defined in the config.
    /// Failures for individual servers are logged as warnings and skipped.
    pub async fn from_config(config: &McpConfig) -> Self {
        let mut servers = Vec::new();

        for (name, server_config) in &config.servers {
            match Self::connect_server(name, server_config).await {
                Ok(handle) => servers.push(handle),
                Err(e) => {
                    eprintln!("[mcp] warning: failed to connect to '{}': {}", name, e);
                }
            }
        }

        McpManager { servers }
    }

    async fn connect_server(name: &str, config: &ServerConfig) -> crate::Result<McpServerHandle> {
        let mut client_kind = match config {
            ServerConfig::Stdio { command, args, env } => {
                let env_map: HashMap<String, String> = env.iter().cloned().collect();
                let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                let transport = StdioTransport::spawn(command, &args_refs, &env_map)?;
                McpClientKind::Stdio(McpClient::new(transport))
            }
            ServerConfig::Sse { .. }
            | ServerConfig::Http { .. }
            | ServerConfig::WebSocket { .. } => {
                return Err(Error::Mcp {
                    server: name.to_string(),
                    message: "only stdio transport is currently supported".to_string(),
                });
            }
        };

        // Initialize the MCP connection
        match &mut client_kind {
            McpClientKind::Stdio(c) => {
                c.initialize().await?;
            }
        }

        // Discover capabilities
        let tools = match &mut client_kind {
            McpClientKind::Stdio(c) => c.list_tools().await.unwrap_or_default(),
        };
        let resources = match &mut client_kind {
            McpClientKind::Stdio(c) => c.list_resources().await.unwrap_or_default(),
        };
        let prompts = match &mut client_kind {
            McpClientKind::Stdio(c) => c.list_prompts().await.unwrap_or_default(),
        };

        Ok(McpServerHandle {
            name: name.to_string(),
            tools,
            resources,
            prompts,
            client: client_kind,
        })
    }

    /// Call a tool on a specific MCP server.
    pub async fn call_tool(
        &mut self,
        server: &str,
        tool: &str,
        args: &JsonValue,
    ) -> crate::Result<String> {
        let handle = self
            .servers
            .iter_mut()
            .find(|s| s.name == server)
            .ok_or_else(|| Error::Mcp {
                server: server.to_string(),
                message: format!("server '{}' not found", server),
            })?;

        let result = handle.client.call_tool(tool, args).await?;
        if result.is_error {
            Err(Error::Mcp {
                server: server.to_string(),
                message: result.to_text(),
            })
        } else {
            Ok(result.to_text())
        }
    }

    /// Read a resource from a specific MCP server.
    pub async fn read_resource(&mut self, server: &str, uri: &str) -> crate::Result<String> {
        let handle = self
            .servers
            .iter_mut()
            .find(|s| s.name == server)
            .ok_or_else(|| Error::Mcp {
                server: server.to_string(),
                message: format!("server '{}' not found", server),
            })?;

        let result = handle.client.read_resource(uri).await?;
        let parts: Vec<String> = result
            .contents
            .iter()
            .map(|item| match item {
                ResourceContentItem::Text { text, .. } => text.clone(),
                ResourceContentItem::Blob { blob, .. } => format!("[blob: {} bytes]", blob.len()),
            })
            .collect();
        Ok(parts.join("\n"))
    }

    /// Get a prompt from a specific MCP server.
    pub async fn get_prompt(
        &mut self,
        server: &str,
        name: &str,
        args: &JsonValue,
    ) -> crate::Result<String> {
        let handle = self
            .servers
            .iter_mut()
            .find(|s| s.name == server)
            .ok_or_else(|| Error::Mcp {
                server: server.to_string(),
                message: format!("server '{}' not found", server),
            })?;

        let result = handle.client.get_prompt(name, args).await?;
        let mut output = String::new();
        if let Some(desc) = &result.description {
            output.push_str(desc);
            output.push('\n');
        }
        for msg in &result.messages {
            output.push_str(&format!("[{}] ", msg.role));
            match &msg.content {
                ContentItem::Text(t) => output.push_str(t),
                ContentItem::Image { mime_type, .. } => {
                    output.push_str(&format!("[image: {}]", mime_type));
                }
                ContentItem::Resource { uri, text } => {
                    output.push_str(&format!("[resource: {}]\n{}", uri, text));
                }
            }
            output.push('\n');
        }
        Ok(output)
    }

    /// Shut down all connected MCP servers.
    pub async fn shutdown_all(&mut self) {
        for handle in &mut self.servers {
            let _ = handle.client.close().await;
        }
    }
}
