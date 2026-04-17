use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;

use viv::core::json::{JsonValue, Number};
use viv::core::runtime::block_on;
use viv::mcp::client::McpClient;
use viv::mcp::transport::Transport;

/// Mock transport for testing the MCP client.
/// Records sent messages and returns pre-configured responses.
struct MockTransport {
    sent: Vec<JsonValue>,
    responses: VecDeque<JsonValue>,
}

impl MockTransport {
    fn new(responses: Vec<JsonValue>) -> Self {
        MockTransport {
            sent: Vec::new(),
            responses: VecDeque::from(responses),
        }
    }
}

impl Transport for MockTransport {
    fn send(
        &mut self,
        msg: JsonValue,
    ) -> Pin<Box<dyn Future<Output = viv::Result<()>> + Send + '_>> {
        self.sent.push(msg);
        Box::pin(async { Ok(()) })
    }

    fn recv(&mut self) -> Pin<Box<dyn Future<Output = viv::Result<JsonValue>> + Send + '_>> {
        let response = self.responses.pop_front().ok_or_else(|| viv::Error::Mcp {
            server: "mock".to_string(),
            message: "No more mock responses".to_string(),
        });
        Box::pin(async move { response })
    }

    fn close(&mut self) -> Pin<Box<dyn Future<Output = viv::Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }
}

/// Helper: build a JSON-RPC success response
fn success_response(id: i64, result: JsonValue) -> JsonValue {
    JsonValue::Object(vec![
        ("jsonrpc".to_string(), JsonValue::Str("2.0".to_string())),
        ("id".to_string(), JsonValue::Number(Number::Int(id))),
        ("result".to_string(), result),
    ])
}

/// Helper: build a JSON-RPC error response
fn error_response(id: i64, code: i64, message: &str) -> JsonValue {
    JsonValue::Object(vec![
        ("jsonrpc".to_string(), JsonValue::Str("2.0".to_string())),
        ("id".to_string(), JsonValue::Number(Number::Int(id))),
        (
            "error".to_string(),
            JsonValue::Object(vec![
                ("code".to_string(), JsonValue::Number(Number::Int(code))),
                ("message".to_string(), JsonValue::Str(message.to_string())),
            ]),
        ),
    ])
}

#[test]
fn initialize_handshake() {
    // Server responds with capabilities
    let init_result = JsonValue::Object(vec![
        (
            "protocolVersion".to_string(),
            JsonValue::Str("2025-11-25".to_string()),
        ),
        (
            "capabilities".to_string(),
            JsonValue::Object(vec![
                ("tools".to_string(), JsonValue::Object(Vec::new())),
                ("resources".to_string(), JsonValue::Object(Vec::new())),
            ]),
        ),
        (
            "serverInfo".to_string(),
            JsonValue::Object(vec![
                (
                    "name".to_string(),
                    JsonValue::Str("test-server".to_string()),
                ),
                ("version".to_string(), JsonValue::Str("1.0.0".to_string())),
            ]),
        ),
    ]);

    let transport = MockTransport::new(vec![success_response(1, init_result)]);

    let mut client = McpClient::new(transport);

    // Run initialize and get back the client + result
    let (caps, client) = block_on(async move {
        let caps = client.initialize().await.unwrap();
        (caps, client)
    });

    // Verify capabilities parsed correctly
    assert!(caps.tools);
    assert!(caps.resources);
    assert!(!caps.prompts);

    // Verify server_capabilities is stored
    assert!(client.server_capabilities.is_some());

    // Verify that two messages were sent: initialize request + initialized notification
    let transport = client.into_transport();
    assert_eq!(transport.sent.len(), 2);

    // First message: initialize request
    let init_req = &transport.sent[0];
    assert_eq!(
        init_req.get("method").and_then(|v| v.as_str()),
        Some("initialize")
    );
    assert_eq!(init_req.get("id").and_then(|v| v.as_i64()), Some(1));

    // Verify params contain protocolVersion and clientInfo
    let params = init_req.get("params").unwrap();
    assert_eq!(
        params.get("protocolVersion").and_then(|v| v.as_str()),
        Some("2025-11-25")
    );
    let client_info = params.get("clientInfo").unwrap();
    assert_eq!(
        client_info.get("name").and_then(|v| v.as_str()),
        Some("viv")
    );
    assert_eq!(
        client_info.get("version").and_then(|v| v.as_str()),
        Some("0.1.0")
    );

    // Second message: notifications/initialized notification (no id)
    let notif = &transport.sent[1];
    assert_eq!(
        notif.get("method").and_then(|v| v.as_str()),
        Some("notifications/initialized")
    );
    // Notification should not have an id field
    assert!(notif.get("id").is_none());
}

#[test]
fn list_tools() {
    let tools_result = JsonValue::Object(vec![(
        "tools".to_string(),
        JsonValue::Array(vec![
            JsonValue::Object(vec![
                ("name".to_string(), JsonValue::Str("read_file".to_string())),
                (
                    "description".to_string(),
                    JsonValue::Str("Read a file".to_string()),
                ),
                (
                    "inputSchema".to_string(),
                    JsonValue::Object(vec![
                        ("type".to_string(), JsonValue::Str("object".to_string())),
                        (
                            "properties".to_string(),
                            JsonValue::Object(vec![(
                                "path".to_string(),
                                JsonValue::Object(vec![(
                                    "type".to_string(),
                                    JsonValue::Str("string".to_string()),
                                )]),
                            )]),
                        ),
                        (
                            "required".to_string(),
                            JsonValue::Array(vec![JsonValue::Str("path".to_string())]),
                        ),
                    ]),
                ),
            ]),
            JsonValue::Object(vec![
                ("name".to_string(), JsonValue::Str("write_file".to_string())),
                (
                    "description".to_string(),
                    JsonValue::Str("Write a file".to_string()),
                ),
                (
                    "inputSchema".to_string(),
                    JsonValue::Object(vec![
                        ("type".to_string(), JsonValue::Str("object".to_string())),
                        (
                            "properties".to_string(),
                            JsonValue::Object(vec![
                                (
                                    "path".to_string(),
                                    JsonValue::Object(vec![(
                                        "type".to_string(),
                                        JsonValue::Str("string".to_string()),
                                    )]),
                                ),
                                (
                                    "content".to_string(),
                                    JsonValue::Object(vec![(
                                        "type".to_string(),
                                        JsonValue::Str("string".to_string()),
                                    )]),
                                ),
                            ]),
                        ),
                        (
                            "required".to_string(),
                            JsonValue::Array(vec![
                                JsonValue::Str("path".to_string()),
                                JsonValue::Str("content".to_string()),
                            ]),
                        ),
                    ]),
                ),
            ]),
        ]),
    )]);

    let transport = MockTransport::new(vec![success_response(1, tools_result)]);

    let mut client = McpClient::new(transport);

    let (tools, transport) = block_on(async move {
        let tools = client.list_tools().await.unwrap();
        (tools, client.into_transport())
    });

    assert_eq!(tools.len(), 2);

    assert_eq!(tools[0].name, "read_file");
    assert_eq!(tools[0].description.as_deref(), Some("Read a file"));
    assert_eq!(tools[0].input_schema.required, vec!["path".to_string()]);

    assert_eq!(tools[1].name, "write_file");
    assert_eq!(tools[1].description.as_deref(), Some("Write a file"));
    assert_eq!(
        tools[1].input_schema.required,
        vec!["path".to_string(), "content".to_string()]
    );

    // Verify the request was sent correctly
    assert_eq!(transport.sent.len(), 1);
    let req = &transport.sent[0];
    assert_eq!(
        req.get("method").and_then(|v| v.as_str()),
        Some("tools/list")
    );
}

#[test]
fn list_tools_with_pagination() {
    // First page with cursor
    let page1 = JsonValue::Object(vec![
        (
            "tools".to_string(),
            JsonValue::Array(vec![JsonValue::Object(vec![(
                "name".to_string(),
                JsonValue::Str("tool_a".to_string()),
            )])]),
        ),
        (
            "nextCursor".to_string(),
            JsonValue::Str("cursor_1".to_string()),
        ),
    ]);

    // Second page without cursor (last page)
    let page2 = JsonValue::Object(vec![(
        "tools".to_string(),
        JsonValue::Array(vec![JsonValue::Object(vec![(
            "name".to_string(),
            JsonValue::Str("tool_b".to_string()),
        )])]),
    )]);

    let transport =
        MockTransport::new(vec![success_response(1, page1), success_response(2, page2)]);

    let mut client = McpClient::new(transport);

    let (tools, transport) = block_on(async move {
        let tools = client.list_tools().await.unwrap();
        (tools, client.into_transport())
    });

    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].name, "tool_a");
    assert_eq!(tools[1].name, "tool_b");

    // Two requests should have been sent
    assert_eq!(transport.sent.len(), 2);

    // Second request should include cursor
    let second_req = &transport.sent[1];
    let params = second_req.get("params").unwrap();
    assert_eq!(
        params.get("cursor").and_then(|v| v.as_str()),
        Some("cursor_1")
    );
}

#[test]
fn call_tool() {
    let call_result = JsonValue::Object(vec![
        (
            "content".to_string(),
            JsonValue::Array(vec![JsonValue::Object(vec![
                ("type".to_string(), JsonValue::Str("text".to_string())),
                (
                    "text".to_string(),
                    JsonValue::Str("file contents here".to_string()),
                ),
            ])]),
        ),
        ("isError".to_string(), JsonValue::Bool(false)),
    ]);

    let transport = MockTransport::new(vec![success_response(1, call_result)]);

    let mut client = McpClient::new(transport);

    let args = JsonValue::Object(vec![(
        "path".to_string(),
        JsonValue::Str("/tmp/test.txt".to_string()),
    )]);

    let (result, transport) = block_on(async move {
        let result = client.call_tool("read_file", &args).await.unwrap();
        (result, client.into_transport())
    });

    assert!(!result.is_error);
    assert_eq!(result.content.len(), 1);
    assert_eq!(result.content[0].content_type, "text");
    assert_eq!(result.content[0].text, "file contents here");

    // Verify request
    let req = &transport.sent[0];
    assert_eq!(
        req.get("method").and_then(|v| v.as_str()),
        Some("tools/call")
    );
    let params = req.get("params").unwrap();
    assert_eq!(
        params.get("name").and_then(|v| v.as_str()),
        Some("read_file")
    );
}

#[test]
fn jsonrpc_error_propagation() {
    let transport = MockTransport::new(vec![error_response(1, -32601, "Method not found")]);

    let mut client = McpClient::new(transport);

    let result = block_on(async move { client.list_tools().await });

    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        viv::Error::JsonRpc { code, message } => {
            assert_eq!(code, -32601);
            assert_eq!(message, "Method not found");
        }
        other => panic!("Expected JsonRpc error, got: {:?}", other),
    }
}

#[test]
fn notifications_buffered_during_request() {
    // Server sends a notification before the response
    let notification = JsonValue::Object(vec![
        ("jsonrpc".to_string(), JsonValue::Str("2.0".to_string())),
        (
            "method".to_string(),
            JsonValue::Str("notifications/progress".to_string()),
        ),
        (
            "params".to_string(),
            JsonValue::Object(vec![(
                "progress".to_string(),
                JsonValue::Number(Number::Int(50)),
            )]),
        ),
    ]);

    let tools_result = JsonValue::Object(vec![("tools".to_string(), JsonValue::Array(Vec::new()))]);

    let transport = MockTransport::new(vec![notification, success_response(1, tools_result)]);

    let mut client = McpClient::new(transport);

    let (tools, notifications) = block_on(async move {
        let tools = client.list_tools().await.unwrap();
        let notifications = client.drain_notifications();
        (tools, notifications)
    });

    assert_eq!(tools.len(), 0);

    // The notification should have been buffered
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].method, "notifications/progress");
}

#[test]
fn shutdown() {
    let transport = MockTransport::new(vec![]);
    let mut client = McpClient::new(transport);

    block_on(async move {
        client.shutdown().await.unwrap();
    });
}

#[test]
fn list_resources() {
    let resources_result = JsonValue::Object(vec![(
        "resources".to_string(),
        JsonValue::Array(vec![JsonValue::Object(vec![
            (
                "uri".to_string(),
                JsonValue::Str("file:///tmp/test.txt".to_string()),
            ),
            ("name".to_string(), JsonValue::Str("test.txt".to_string())),
            (
                "mimeType".to_string(),
                JsonValue::Str("text/plain".to_string()),
            ),
        ])]),
    )]);

    let transport = MockTransport::new(vec![success_response(1, resources_result)]);

    let mut client = McpClient::new(transport);

    let resources = block_on(async move { client.list_resources().await.unwrap() });

    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].uri, "file:///tmp/test.txt");
    assert_eq!(resources[0].name, "test.txt");
    assert_eq!(resources[0].mime_type.as_deref(), Some("text/plain"));
}

#[test]
fn read_resource() {
    let read_result = JsonValue::Object(vec![(
        "contents".to_string(),
        JsonValue::Array(vec![JsonValue::Object(vec![
            (
                "uri".to_string(),
                JsonValue::Str("file:///tmp/test.txt".to_string()),
            ),
            (
                "mimeType".to_string(),
                JsonValue::Str("text/plain".to_string()),
            ),
            (
                "text".to_string(),
                JsonValue::Str("hello world".to_string()),
            ),
        ])]),
    )]);

    let transport = MockTransport::new(vec![success_response(1, read_result)]);

    let mut client = McpClient::new(transport);

    let content =
        block_on(async move { client.read_resource("file:///tmp/test.txt").await.unwrap() });

    assert_eq!(content.uri, "file:///tmp/test.txt");
    assert_eq!(content.text.as_deref(), Some("hello world"));
    assert_eq!(content.mime_type.as_deref(), Some("text/plain"));
}

#[test]
fn list_prompts() {
    let prompts_result = JsonValue::Object(vec![(
        "prompts".to_string(),
        JsonValue::Array(vec![JsonValue::Object(vec![
            (
                "name".to_string(),
                JsonValue::Str("code_review".to_string()),
            ),
            (
                "description".to_string(),
                JsonValue::Str("Review code".to_string()),
            ),
            (
                "arguments".to_string(),
                JsonValue::Array(vec![JsonValue::Object(vec![
                    ("name".to_string(), JsonValue::Str("code".to_string())),
                    (
                        "description".to_string(),
                        JsonValue::Str("The code to review".to_string()),
                    ),
                    ("required".to_string(), JsonValue::Bool(true)),
                ])]),
            ),
        ])]),
    )]);

    let transport = MockTransport::new(vec![success_response(1, prompts_result)]);

    let mut client = McpClient::new(transport);

    let prompts = block_on(async move { client.list_prompts().await.unwrap() });

    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].name, "code_review");
    assert_eq!(prompts[0].description.as_deref(), Some("Review code"));
    assert_eq!(prompts[0].arguments.len(), 1);
    assert_eq!(prompts[0].arguments[0].name, "code");
    assert!(prompts[0].arguments[0].required);
}

#[test]
fn get_prompt() {
    let prompt_result = JsonValue::Object(vec![
        (
            "description".to_string(),
            JsonValue::Str("Code review prompt".to_string()),
        ),
        (
            "messages".to_string(),
            JsonValue::Array(vec![JsonValue::Object(vec![
                ("role".to_string(), JsonValue::Str("user".to_string())),
                (
                    "content".to_string(),
                    JsonValue::Str("Please review this code".to_string()),
                ),
            ])]),
        ),
    ]);

    let transport = MockTransport::new(vec![success_response(1, prompt_result)]);

    let mut client = McpClient::new(transport);

    let args = JsonValue::Object(vec![(
        "code".to_string(),
        JsonValue::Str("fn main() {}".to_string()),
    )]);

    let result = block_on(async move { client.get_prompt("code_review", &args).await.unwrap() });

    assert_eq!(result.description.as_deref(), Some("Code review prompt"));
    assert_eq!(result.messages.len(), 1);
    assert_eq!(result.messages[0].role, "user");
    assert_eq!(result.messages[0].content, "Please review this code");
}
