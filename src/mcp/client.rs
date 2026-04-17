use crate::Error;
use crate::core::json::JsonValue;
use crate::core::jsonrpc::{Message, Notification, Request, Response};
use crate::mcp::transport::Transport;
use crate::mcp::types::*;

/// MCP client that handles the JSON-RPC protocol lifecycle over any Transport.
pub struct McpClient<T: Transport> {
    transport: T,
    next_id: i64,
    pub server_capabilities: Option<ServerCapabilities>,
    pending_notifications: Vec<Notification>,
}

impl<T: Transport> McpClient<T> {
    pub fn new(transport: T) -> Self {
        McpClient {
            transport,
            next_id: 1,
            server_capabilities: None,
            pending_notifications: Vec::new(),
        }
    }

    /// Send a JSON-RPC request and wait for the matching response.
    /// Notifications received while waiting are buffered.
    async fn request(
        &mut self,
        method: &str,
        params: Option<JsonValue>,
    ) -> crate::Result<JsonValue> {
        let id = self.next_id;
        self.next_id += 1;
        let req = Request {
            id,
            method: method.to_string(),
            params,
        };
        self.transport.send(req.to_json()).await?;
        loop {
            let msg_json = self.transport.recv().await?;
            let msg = Message::parse(&msg_json)?;
            match msg {
                Message::Response(Response::Result { id: rid, result }) if rid == id => {
                    return Ok(result);
                }
                Message::Response(Response::Error { id: rid, error }) if rid == id => {
                    return Err(Error::JsonRpc {
                        code: error.code,
                        message: error.message,
                    });
                }
                Message::Notification(n) => {
                    self.pending_notifications.push(n);
                }
                _ => {
                    // Ignore non-matching responses (could be from a different request)
                }
            }
        }
    }

    /// Send a JSON-RPC notification (no response expected).
    async fn notify(&mut self, method: &str, params: Option<JsonValue>) -> crate::Result<()> {
        let notif = Notification {
            method: method.to_string(),
            params,
        };
        self.transport.send(notif.to_json()).await
    }

    /// Perform the MCP initialize handshake.
    ///
    /// Sends an `initialize` request with protocol version and client info,
    /// then sends a `notifications/initialized` notification.
    pub async fn initialize(&mut self) -> crate::Result<ServerCapabilities> {
        let params = JsonValue::Object(vec![
            (
                "protocolVersion".to_string(),
                JsonValue::Str("2025-11-25".to_string()),
            ),
            (
                "clientInfo".to_string(),
                JsonValue::Object(vec![
                    ("name".to_string(), JsonValue::Str("viv".to_string())),
                    ("version".to_string(), JsonValue::Str("0.1.0".to_string())),
                ]),
            ),
            ("capabilities".to_string(), JsonValue::Object(Vec::new())),
        ]);

        let result = self.request("initialize", Some(params)).await?;
        let caps_json = result.get("capabilities").unwrap_or(&result);
        let caps = ServerCapabilities::from_json(caps_json)?;
        self.server_capabilities = Some(caps.clone());

        // Send initialized notification
        self.notify("notifications/initialized", None).await?;

        Ok(caps)
    }

    /// Shut down the client by closing the transport.
    pub async fn shutdown(&mut self) -> crate::Result<()> {
        self.transport.close().await
    }

    /// List all available tools, handling cursor-based pagination.
    pub async fn list_tools(&mut self) -> crate::Result<Vec<McpTool>> {
        let mut all_tools = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let params = cursor.as_ref().map(|c| {
                JsonValue::Object(vec![("cursor".to_string(), JsonValue::Str(c.clone()))])
            });

            let result = self.request("tools/list", params).await?;
            let result_obj = JsonValue::Object(vec![("tools".into(), result.get("tools").cloned().unwrap_or(JsonValue::Array(vec![])))]);
            all_tools.extend(McpTool::parse_list(&result_obj)?);

            // Check for next cursor
            match result.get("nextCursor").and_then(|v| v.as_str()) {
                Some(next) if !next.is_empty() => {
                    cursor = Some(next.to_string());
                }
                _ => break,
            }
        }

        Ok(all_tools)
    }

    /// Call a tool by name with the given arguments.
    pub async fn call_tool(
        &mut self,
        name: &str,
        args: &JsonValue,
    ) -> crate::Result<ToolCallResult> {
        let params = JsonValue::Object(vec![
            ("name".to_string(), JsonValue::Str(name.to_string())),
            ("arguments".to_string(), args.clone()),
        ]);

        let result = self.request("tools/call", Some(params)).await?;
        ToolCallResult::from_json(&result)
    }

    /// List all available resources, handling cursor-based pagination.
    pub async fn list_resources(&mut self) -> crate::Result<Vec<McpResource>> {
        let mut all_resources = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let params = cursor.as_ref().map(|c| {
                JsonValue::Object(vec![("cursor".to_string(), JsonValue::Str(c.clone()))])
            });

            let result = self.request("resources/list", params).await?;
            let result_obj = JsonValue::Object(vec![("resources".into(), result.get("resources").cloned().unwrap_or(JsonValue::Array(vec![])))]);
            all_resources.extend(McpResource::parse_list(&result_obj)?);

            match result.get("nextCursor").and_then(|v| v.as_str()) {
                Some(next) if !next.is_empty() => {
                    cursor = Some(next.to_string());
                }
                _ => break,
            }
        }

        Ok(all_resources)
    }

    /// Read a resource by URI.
    pub async fn read_resource(&mut self, uri: &str) -> crate::Result<ResourceContent> {
        let params = JsonValue::Object(vec![("uri".to_string(), JsonValue::Str(uri.to_string()))]);

        let result = self.request("resources/read", Some(params)).await?;
        ResourceContent::from_json(&result)
    }

    /// List all available prompts, handling cursor-based pagination.
    pub async fn list_prompts(&mut self) -> crate::Result<Vec<McpPrompt>> {
        let mut all_prompts = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let params = cursor.as_ref().map(|c| {
                JsonValue::Object(vec![("cursor".to_string(), JsonValue::Str(c.clone()))])
            });

            let result = self.request("prompts/list", params).await?;
            let result_obj = JsonValue::Object(vec![("prompts".into(), result.get("prompts").cloned().unwrap_or(JsonValue::Array(vec![])))]);
            all_prompts.extend(McpPrompt::parse_list(&result_obj)?);

            match result.get("nextCursor").and_then(|v| v.as_str()) {
                Some(next) if !next.is_empty() => {
                    cursor = Some(next.to_string());
                }
                _ => break,
            }
        }

        Ok(all_prompts)
    }

    /// Get a prompt by name with arguments.
    pub async fn get_prompt(
        &mut self,
        name: &str,
        args: &JsonValue,
    ) -> crate::Result<PromptMessages> {
        let params = JsonValue::Object(vec![
            ("name".to_string(), JsonValue::Str(name.to_string())),
            ("arguments".to_string(), args.clone()),
        ]);

        let result = self.request("prompts/get", Some(params)).await?;
        PromptMessages::from_json(&result)
    }

    /// Drain any notifications that arrived while waiting for responses.
    pub fn drain_notifications(&mut self) -> Vec<Notification> {
        std::mem::take(&mut self.pending_notifications)
    }

    /// Consume the client and return the underlying transport.
    /// Useful for testing to inspect sent messages.
    pub fn into_transport(self) -> T {
        self.transport
    }
}
