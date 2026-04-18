use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;

use viv::core::json::{JsonValue, Number};
use viv::core::runtime::block_on;
use viv::mcp::client::McpClient;
use viv::mcp::transport::Transport;

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
            server: "mock".into(),
            message: "no response".into(),
        });
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

fn init_result() -> JsonValue {
    JsonValue::Object(vec![
        (
            "protocolVersion".into(),
            JsonValue::Str("2025-11-25".into()),
        ),
        (
            "capabilities".into(),
            JsonValue::Object(vec![("tools".into(), JsonValue::Object(vec![]))]),
        ),
        (
            "serverInfo".into(),
            JsonValue::Object(vec![
                ("name".into(), JsonValue::Str("test".into())),
                ("version".into(), JsonValue::Str("1.0".into())),
            ]),
        ),
    ])
}

#[test]
fn initialize_handshake() {
    let transport = MockTransport::new(vec![success(1, init_result())]);
    let mut client = McpClient::new(transport);

    let (caps, client) = block_on(async move {
        let caps = client.initialize().await.unwrap();
        (caps, client)
    });

    assert!(caps.tools.is_some());
    assert!(caps.resources.is_none());
    assert!(caps.prompts.is_none());
    assert!(client.server_capabilities.is_some());

    let transport = client.into_transport();
    assert_eq!(transport.sent.len(), 2);
    assert_eq!(
        transport.sent[0].get("method").unwrap().as_str().unwrap(),
        "initialize"
    );
    assert_eq!(
        transport.sent[1].get("method").unwrap().as_str().unwrap(),
        "notifications/initialized"
    );
    assert!(transport.sent[1].get("id").is_none());
}

#[test]
fn list_tools() {
    let result = JsonValue::Object(vec![(
        "tools".into(),
        JsonValue::Array(vec![JsonValue::Object(vec![
            ("name".into(), JsonValue::Str("read_file".into())),
            ("description".into(), JsonValue::Str("Read a file".into())),
            (
                "inputSchema".into(),
                JsonValue::Object(vec![("type".into(), JsonValue::Str("object".into()))]),
            ),
        ])]),
    )]);

    let transport = MockTransport::new(vec![success(1, result)]);
    let mut client = McpClient::new(transport);

    let tools = block_on(async move { client.list_tools().await.unwrap() });
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "read_file");
    assert_eq!(tools[0].description.as_deref(), Some("Read a file"));
}

#[test]
fn list_tools_with_pagination() {
    let page1 = JsonValue::Object(vec![
        (
            "tools".into(),
            JsonValue::Array(vec![JsonValue::Object(vec![(
                "name".into(),
                JsonValue::Str("tool_a".into()),
            )])]),
        ),
        ("nextCursor".into(), JsonValue::Str("cursor_1".into())),
    ]);
    let page2 = JsonValue::Object(vec![(
        "tools".into(),
        JsonValue::Array(vec![JsonValue::Object(vec![(
            "name".into(),
            JsonValue::Str("tool_b".into()),
        )])]),
    )]);

    let transport = MockTransport::new(vec![success(1, page1), success(2, page2)]);
    let mut client = McpClient::new(transport);

    let (tools, transport) = block_on(async move {
        let tools = client.list_tools().await.unwrap();
        (tools, client.into_transport())
    });

    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].name, "tool_a");
    assert_eq!(tools[1].name, "tool_b");
    assert_eq!(transport.sent.len(), 2);
}

#[test]
fn call_tool() {
    let result = JsonValue::Object(vec![
        (
            "content".into(),
            JsonValue::Array(vec![JsonValue::Object(vec![
                ("type".into(), JsonValue::Str("text".into())),
                ("text".into(), JsonValue::Str("file contents".into())),
            ])]),
        ),
        ("isError".into(), JsonValue::Bool(false)),
    ]);

    let transport = MockTransport::new(vec![success(1, result)]);
    let mut client = McpClient::new(transport);
    let args = JsonValue::Object(vec![("path".into(), JsonValue::Str("/tmp/test".into()))]);

    let result = block_on(async move { client.call_tool("read_file", &args).await.unwrap() });
    assert!(!result.is_error);
    assert_eq!(result.to_text(), "file contents");
}

#[test]
fn jsonrpc_error_propagation() {
    let err_resp = JsonValue::Object(vec![
        ("jsonrpc".into(), JsonValue::Str("2.0".into())),
        ("id".into(), JsonValue::Number(Number::Int(1))),
        (
            "error".into(),
            JsonValue::Object(vec![
                ("code".into(), JsonValue::Number(Number::Int(-32601))),
                ("message".into(), JsonValue::Str("Method not found".into())),
            ]),
        ),
    ]);

    let transport = MockTransport::new(vec![err_resp]);
    let mut client = McpClient::new(transport);

    let result = block_on(async move { client.list_tools().await });
    match result.unwrap_err() {
        viv::Error::JsonRpc { code, message } => {
            assert_eq!(code, -32601);
            assert_eq!(message, "Method not found");
        }
        other => panic!("expected JsonRpc error, got: {:?}", other),
    }
}

#[test]
fn notifications_buffered() {
    let notif = JsonValue::Object(vec![
        ("jsonrpc".into(), JsonValue::Str("2.0".into())),
        (
            "method".into(),
            JsonValue::Str("notifications/progress".into()),
        ),
    ]);
    let result = JsonValue::Object(vec![("tools".into(), JsonValue::Array(vec![]))]);

    let transport = MockTransport::new(vec![notif, success(1, result)]);
    let mut client = McpClient::new(transport);

    let (tools, notifs) = block_on(async move {
        let tools = client.list_tools().await.unwrap();
        let notifs = client.drain_notifications();
        (tools, notifs)
    });

    assert!(tools.is_empty());
    assert_eq!(notifs.len(), 1);
    assert_eq!(notifs[0].method, "notifications/progress");
}

#[test]
fn list_resources() {
    let result = JsonValue::Object(vec![(
        "resources".into(),
        JsonValue::Array(vec![JsonValue::Object(vec![
            ("uri".into(), JsonValue::Str("file:///tmp/test.txt".into())),
            ("name".into(), JsonValue::Str("test.txt".into())),
            ("mimeType".into(), JsonValue::Str("text/plain".into())),
        ])]),
    )]);

    let transport = MockTransport::new(vec![success(1, result)]);
    let mut client = McpClient::new(transport);

    let resources = block_on(async move { client.list_resources().await.unwrap() });
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].uri, "file:///tmp/test.txt");
    assert_eq!(resources[0].mime_type.as_deref(), Some("text/plain"));
}

#[test]
fn list_prompts() {
    let result = JsonValue::Object(vec![(
        "prompts".into(),
        JsonValue::Array(vec![JsonValue::Object(vec![
            ("name".into(), JsonValue::Str("code_review".into())),
            ("description".into(), JsonValue::Str("Review code".into())),
            (
                "arguments".into(),
                JsonValue::Array(vec![JsonValue::Object(vec![
                    ("name".into(), JsonValue::Str("code".into())),
                    ("required".into(), JsonValue::Bool(true)),
                ])]),
            ),
        ])]),
    )]);

    let transport = MockTransport::new(vec![success(1, result)]);
    let mut client = McpClient::new(transport);

    let prompts = block_on(async move { client.list_prompts().await.unwrap() });
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].name, "code_review");
    assert_eq!(prompts[0].arguments.len(), 1);
    assert!(prompts[0].arguments[0].required);
}

#[test]
fn shutdown() {
    let transport = MockTransport::new(vec![]);
    let mut client = McpClient::new(transport);
    block_on(async move { client.shutdown().await.unwrap() });
}
