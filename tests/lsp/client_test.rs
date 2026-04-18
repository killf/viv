use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;

use viv::core::json::{JsonValue, Number};
use viv::core::runtime::block_on;
use viv::lsp::client::LspClient;
use viv::mcp::transport::Transport;

// ---------------------------------------------------------------------------
// MockTransport
// ---------------------------------------------------------------------------

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
    fn send(
        &mut self,
        msg: JsonValue,
    ) -> Pin<Box<dyn Future<Output = viv::Result<()>> + Send + '_>> {
        self.sent.push(msg);
        Box::pin(async { Ok(()) })
    }

    fn recv(&mut self) -> Pin<Box<dyn Future<Output = viv::Result<JsonValue>> + Send + '_>> {
        let response = self.responses.pop_front().ok_or_else(|| viv::Error::Lsp {
            server: "mock".into(),
            message: "no response queued".into(),
        });
        Box::pin(async move { response })
    }

    fn close(&mut self) -> Pin<Box<dyn Future<Output = viv::Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn success(id: i64, result: JsonValue) -> JsonValue {
    JsonValue::Object(vec![
        ("jsonrpc".into(), JsonValue::Str("2.0".into())),
        ("id".into(), JsonValue::Number(Number::Int(id))),
        ("result".into(), result),
    ])
}

fn notification(method: &str, params: JsonValue) -> JsonValue {
    JsonValue::Object(vec![
        ("jsonrpc".into(), JsonValue::Str("2.0".into())),
        ("method".into(), JsonValue::Str(method.into())),
        ("params".into(), params),
    ])
}

/// A minimal `initialize` response with definitionProvider, referencesProvider, hoverProvider.
fn init_result() -> JsonValue {
    JsonValue::Object(vec![("capabilities".into(), JsonValue::Object(vec![
        ("definitionProvider".into(), JsonValue::Bool(true)),
        ("referencesProvider".into(), JsonValue::Bool(true)),
        ("hoverProvider".into(), JsonValue::Bool(true)),
    ]))])
}

fn make_range(
    start_line: i64,
    start_char: i64,
    end_line: i64,
    end_char: i64,
) -> JsonValue {
    JsonValue::Object(vec![
        ("start".into(), JsonValue::Object(vec![
            ("line".into(), JsonValue::Number(Number::Int(start_line))),
            ("character".into(), JsonValue::Number(Number::Int(start_char))),
        ])),
        ("end".into(), JsonValue::Object(vec![
            ("line".into(), JsonValue::Number(Number::Int(end_line))),
            ("character".into(), JsonValue::Number(Number::Int(end_char))),
        ])),
    ])
}

fn make_location(uri: &str) -> JsonValue {
    JsonValue::Object(vec![
        ("uri".into(), JsonValue::Str(uri.into())),
        ("range".into(), make_range(0, 0, 0, 5)),
    ])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// `initialize` sends the correct request + `initialized` notification; capabilities are parsed.
#[test]
fn initialize_handshake() {
    let transport = MockTransport::new(vec![success(1, init_result())]);
    let mut client = LspClient::new(transport, "rust-analyzer");

    let (caps, client) = block_on(async move {
        let caps = client.initialize("/workspace").await.unwrap();
        (caps, client)
    });

    assert!(caps.definition);
    assert!(caps.references);
    assert!(caps.hover);
    assert!(client.capabilities.is_some());

    let transport = client.into_transport();

    // Two messages sent: request "initialize" and notification "initialized"
    assert_eq!(transport.sent.len(), 2);

    let init_req = &transport.sent[0];
    assert_eq!(init_req.get("method").unwrap().as_str().unwrap(), "initialize");
    assert!(init_req.get("id").is_some(), "initialize must have an id");

    // rootUri should be a file:// URI
    let root_uri = init_req
        .get("params")
        .and_then(|p| p.get("rootUri"))
        .and_then(|v| v.as_str())
        .unwrap();
    assert!(root_uri.starts_with("file://"), "rootUri should start with file://");

    let init_notif = &transport.sent[1];
    assert_eq!(init_notif.get("method").unwrap().as_str().unwrap(), "initialized");
    assert!(init_notif.get("id").is_none(), "initialized must NOT have an id");
}

/// `initialize` with a path that already has `file://` should not double-prefix.
#[test]
fn initialize_root_uri_already_has_scheme() {
    let transport = MockTransport::new(vec![success(1, init_result())]);
    let mut client = LspClient::new(transport, "rust-analyzer");

    let client = block_on(async move {
        client.initialize("file:///workspace").await.unwrap();
        client
    });

    let transport = client.into_transport();
    let root_uri = transport.sent[0]
        .get("params")
        .and_then(|p| p.get("rootUri"))
        .and_then(|v| v.as_str())
        .unwrap();
    assert_eq!(root_uri, "file:///workspace");
}

/// `definition` sends correct request and parses an array of Locations.
#[test]
fn definition_request() {
    let locations = JsonValue::Array(vec![
        make_location("file:///src/main.rs"),
        make_location("file:///src/lib.rs"),
    ]);

    let transport =
        MockTransport::new(vec![success(1, locations)]);
    let mut client = LspClient::new(transport, "rust-analyzer");

    let locations = block_on(async move {
        client.definition("file:///src/main.rs", 10, 5).await.unwrap()
    });

    assert_eq!(locations.len(), 2);
    assert_eq!(locations[0].uri, "file:///src/main.rs");
    assert_eq!(locations[1].uri, "file:///src/lib.rs");
}

/// `definition` returning a single Location object (not an array) is also handled.
#[test]
fn definition_single_location() {
    let location = make_location("file:///src/main.rs");

    let transport = MockTransport::new(vec![success(1, location)]);
    let mut client = LspClient::new(transport, "rust-analyzer");

    let locations = block_on(async move {
        client.definition("file:///src/main.rs", 3, 0).await.unwrap()
    });

    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0].uri, "file:///src/main.rs");
}

/// `definition` returning null should yield an empty Vec.
#[test]
fn definition_null_result() {
    let transport = MockTransport::new(vec![success(1, JsonValue::Null)]);
    let mut client = LspClient::new(transport, "rust-analyzer");

    let locations = block_on(async move {
        client.definition("file:///src/main.rs", 0, 0).await.unwrap()
    });

    assert!(locations.is_empty());
}

/// `references` sends `includeDeclaration: true` and parses the array.
#[test]
fn references_request() {
    let locations = JsonValue::Array(vec![make_location("file:///src/foo.rs")]);

    let transport = MockTransport::new(vec![success(1, locations)]);
    let mut client = LspClient::new(transport, "rust-analyzer");

    let (refs, client) = block_on(async move {
        let refs = client.references("file:///src/foo.rs", 5, 2).await.unwrap();
        (refs, client)
    });

    assert_eq!(refs.len(), 1);

    let transport = client.into_transport();
    let params = transport.sent[0].get("params").unwrap();
    let include_decl = params
        .get("context")
        .and_then(|c| c.get("includeDeclaration"))
        .and_then(|v| v.as_bool())
        .unwrap();
    assert!(include_decl);
}

/// `hover` returns Some(HoverResult) when the server returns an object.
#[test]
fn hover_request() {
    let hover_resp = JsonValue::Object(vec![(
        "contents".into(),
        JsonValue::Object(vec![
            ("kind".into(), JsonValue::Str("markdown".into())),
            ("value".into(), JsonValue::Str("fn foo() -> u32".into())),
        ]),
    )]);

    let transport = MockTransport::new(vec![success(1, hover_resp)]);
    let mut client = LspClient::new(transport, "rust-analyzer");

    let result =
        block_on(async move { client.hover("file:///src/main.rs", 2, 4).await.unwrap() });

    let hover = result.expect("expected Some(HoverResult)");
    assert_eq!(hover.contents, "fn foo() -> u32");
}

/// `hover` returns None when the server responds with null.
#[test]
fn null_result_for_hover() {
    let transport = MockTransport::new(vec![success(1, JsonValue::Null)]);
    let mut client = LspClient::new(transport, "rust-analyzer");

    let result =
        block_on(async move { client.hover("file:///src/main.rs", 0, 0).await.unwrap() });

    assert!(result.is_none());
}

/// A `textDocument/publishDiagnostics` notification received while waiting for a
/// response is cached and queryable via `diagnostics()`.
#[test]
fn diagnostics_from_notification() {
    let diag_notif = notification(
        "textDocument/publishDiagnostics",
        JsonValue::Object(vec![
            ("uri".into(), JsonValue::Str("file:///src/main.rs".into())),
            ("diagnostics".into(), JsonValue::Array(vec![JsonValue::Object(vec![
                ("range".into(), make_range(3, 0, 3, 10)),
                ("severity".into(), JsonValue::Number(Number::Int(1))),
                ("message".into(), JsonValue::Str("unused variable".into())),
            ])])),
        ]),
    );

    // The notification comes before the actual response.
    let locations = JsonValue::Array(vec![make_location("file:///src/main.rs")]);
    let transport = MockTransport::new(vec![diag_notif, success(1, locations)]);
    let mut client = LspClient::new(transport, "rust-analyzer");

    let (locs, client) = block_on(async move {
        let locs = client.definition("file:///src/main.rs", 0, 0).await.unwrap();
        (locs, client)
    });

    // The definition call should still succeed.
    assert_eq!(locs.len(), 1);

    // The diagnostic should be cached.
    let diags = client.diagnostics("file:///src/main.rs");
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0].message, "unused variable");

    let all = client.all_diagnostics();
    assert_eq!(all.len(), 1);
}

/// `did_open` sends a notification (no id).
#[test]
fn did_open_notification() {
    let transport = MockTransport::new(vec![]);
    let mut client = LspClient::new(transport, "rust-analyzer");

    let client = block_on(async move {
        client
            .did_open("file:///src/main.rs", "rust", "fn main() {}")
            .await
            .unwrap();
        client
    });

    let transport = client.into_transport();
    assert_eq!(transport.sent.len(), 1);
    let msg = &transport.sent[0];
    assert_eq!(msg.get("method").unwrap().as_str().unwrap(), "textDocument/didOpen");
    assert!(msg.get("id").is_none(), "didOpen must NOT have an id");
}

/// `shutdown` sends a `shutdown` request then an `exit` notification, then closes.
#[test]
fn shutdown_sequence() {
    let transport = MockTransport::new(vec![success(1, JsonValue::Null)]);
    let mut client = LspClient::new(transport, "rust-analyzer");

    let client = block_on(async move {
        client.shutdown().await.unwrap();
        client
    });

    let transport = client.into_transport();
    assert_eq!(transport.sent.len(), 2);
    assert_eq!(transport.sent[0].get("method").unwrap().as_str().unwrap(), "shutdown");
    assert!(transport.sent[0].get("id").is_some());
    assert_eq!(transport.sent[1].get("method").unwrap().as_str().unwrap(), "exit");
    assert!(transport.sent[1].get("id").is_none());
}

/// `notify_did_change` sends a notification (no id) with correct params.
#[test]
fn did_change_notification() {
    let transport = MockTransport::new(vec![]);
    let mut client = LspClient::new(transport, "rust-analyzer");

    let client = block_on(async move {
        client
            .notify_did_change("file:///src/main.rs", "fn updated() {}", 2)
            .await
            .unwrap();
        client
    });

    let transport = client.into_transport();
    assert_eq!(transport.sent.len(), 1);
    let msg = &transport.sent[0];
    assert_eq!(msg.get("method").unwrap().as_str().unwrap(), "textDocument/didChange");
    assert!(msg.get("id").is_none(), "didChange must NOT have an id");

    let params = msg.get("params").unwrap();
    let doc = params.get("textDocument").unwrap();
    assert_eq!(doc.get("uri").unwrap().as_str().unwrap(), "file:///src/main.rs");
    assert_eq!(doc.get("version").unwrap().as_i64().unwrap(), 2);

    let changes = params.get("contentChanges").unwrap().as_array().unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].get("text").unwrap().as_str().unwrap(), "fn updated() {}");
}
