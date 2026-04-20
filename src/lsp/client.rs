use std::collections::HashMap;

use crate::Error;
use crate::core::json::JsonValue;
use crate::lsp::jsonrpc::{Message, Notification, Request, Response};
use crate::lsp::types::{Diagnostic, HoverResult, Location, LspServerCapabilities};
use crate::mcp::transport::Transport;

/// LSP client that handles the JSON-RPC protocol lifecycle over any Transport.
pub struct LspClient<T: Transport> {
    transport: T,
    next_id: i64,
    #[allow(dead_code)]
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

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Perform the LSP initialize handshake.
    ///
    /// Sends `initialize` with processId, rootUri, and client capabilities,
    /// then sends the `initialized` notification.
    pub async fn initialize(&mut self, root_path: &str) -> crate::Result<LspServerCapabilities> {
        let root_uri = if root_path.starts_with("file://") {
            root_path.to_string()
        } else {
            format!("file://{}", root_path)
        };

        let params = JsonValue::Object(vec![
            (
                "processId".to_string(),
                JsonValue::Number(crate::core::json::Number::Int(std::process::id() as i64)),
            ),
            ("rootUri".to_string(), JsonValue::Str(root_uri)),
            (
                "capabilities".to_string(),
                JsonValue::Object(vec![(
                    "textDocument".to_string(),
                    JsonValue::Object(vec![
                        (
                            "definition".to_string(),
                            JsonValue::Object(vec![(
                                "dynamicRegistration".to_string(),
                                JsonValue::Bool(false),
                            )]),
                        ),
                        (
                            "references".to_string(),
                            JsonValue::Object(vec![(
                                "dynamicRegistration".to_string(),
                                JsonValue::Bool(false),
                            )]),
                        ),
                        (
                            "hover".to_string(),
                            JsonValue::Object(vec![(
                                "dynamicRegistration".to_string(),
                                JsonValue::Bool(false),
                            )]),
                        ),
                        ("publishDiagnostics".to_string(), JsonValue::Object(vec![])),
                    ]),
                )]),
            ),
        ]);

        let result = self.request("initialize", Some(params)).await?;
        let caps = LspServerCapabilities::from_json(&result)?;
        self.capabilities = Some(caps.clone());

        // Send initialized notification (no response expected)
        self.notify("initialized", None).await?;

        Ok(caps)
    }

    /// Request go-to-definition locations.
    pub async fn definition(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> crate::Result<Vec<Location>> {
        let params = text_document_position(uri, line, character);
        let result = self
            .request("textDocument/definition", Some(params))
            .await?;
        parse_location_response(&result)
    }

    /// Request all references to a symbol.
    pub async fn references(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> crate::Result<Vec<Location>> {
        let mut params_pairs = text_document_position_pairs(uri, line, character);
        params_pairs.push((
            "context".to_string(),
            JsonValue::Object(vec![(
                "includeDeclaration".to_string(),
                JsonValue::Bool(true),
            )]),
        ));
        let params = JsonValue::Object(params_pairs);
        let result = self
            .request("textDocument/references", Some(params))
            .await?;
        parse_location_response(&result)
    }

    /// Request hover information. Returns None if the server responds with null.
    pub async fn hover(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
    ) -> crate::Result<Option<HoverResult>> {
        let params = text_document_position(uri, line, character);
        let result = self.request("textDocument/hover", Some(params)).await?;
        match result {
            JsonValue::Null => Ok(None),
            other => Ok(Some(HoverResult::from_json(&other)?)),
        }
    }

    /// Return cached diagnostics for a specific URI.
    pub fn diagnostics(&self, uri: &str) -> Vec<Diagnostic> {
        self.diagnostics_cache.get(uri).cloned().unwrap_or_default()
    }

    /// Return all cached diagnostics.
    pub fn all_diagnostics(&self) -> &HashMap<String, Vec<Diagnostic>> {
        &self.diagnostics_cache
    }

    /// Send a `textDocument/didOpen` notification.
    pub async fn did_open(
        &mut self,
        uri: &str,
        language_id: &str,
        text: &str,
    ) -> crate::Result<()> {
        let params = JsonValue::Object(vec![(
            "textDocument".to_string(),
            JsonValue::Object(vec![
                ("uri".to_string(), JsonValue::Str(uri.to_string())),
                (
                    "languageId".to_string(),
                    JsonValue::Str(language_id.to_string()),
                ),
                (
                    "version".to_string(),
                    JsonValue::Number(crate::core::json::Number::Int(1)),
                ),
                ("text".to_string(), JsonValue::Str(text.to_string())),
            ]),
        )]);
        self.notify("textDocument/didOpen", Some(params)).await
    }

    /// Send a `textDocument/didChange` notification with full file content.
    pub async fn notify_did_change(
        &mut self,
        uri: &str,
        content: &str,
        version: i32,
    ) -> crate::Result<()> {
        let params = JsonValue::Object(vec![
            (
                "textDocument".to_string(),
                JsonValue::Object(vec![
                    ("uri".to_string(), JsonValue::Str(uri.to_string())),
                    (
                        "version".to_string(),
                        JsonValue::Number(crate::core::json::Number::Int(version as i64)),
                    ),
                ]),
            ),
            (
                "contentChanges".to_string(),
                JsonValue::Array(vec![JsonValue::Object(vec![(
                    "text".to_string(),
                    JsonValue::Str(content.to_string()),
                )])]),
            ),
        ]);
        self.notify("textDocument/didChange", Some(params)).await
    }

    /// Gracefully shut down: send `shutdown` request, send `exit` notification, close transport.
    pub async fn shutdown(&mut self) -> crate::Result<()> {
        self.request("shutdown", None).await?;
        self.notify("exit", None).await?;
        self.transport.close().await
    }

    /// Consume the client and return the underlying transport (useful for testing).
    pub fn into_transport(self) -> T {
        self.transport
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Send a JSON-RPC request and wait for the matching response.
    /// Notifications received while waiting are handled (diagnostics cached, others ignored).
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
                    self.handle_notification(&n);
                }
                _ => {
                    // Ignore non-matching responses
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

    /// Handle an incoming notification. Caches `textDocument/publishDiagnostics`.
    fn handle_notification(&mut self, notif: &Notification) {
        if notif.method != "textDocument/publishDiagnostics" {
            return;
        }
        let params = match &notif.params {
            Some(p) => p,
            None => return,
        };
        let uri = match params.get("uri").and_then(|v| v.as_str()) {
            Some(u) => u.to_string(),
            None => return,
        };
        let diags_json = match params.get("diagnostics") {
            Some(JsonValue::Array(arr)) => arr,
            _ => return,
        };
        let diagnostics: Vec<Diagnostic> = diags_json
            .iter()
            .filter_map(|d| Diagnostic::from_json(d).ok())
            .collect();
        self.diagnostics_cache.insert(uri, diagnostics);
    }
}

// ---------------------------------------------------------------------------
// Free helper functions
// ---------------------------------------------------------------------------

/// Build the base key-value pairs for a TextDocumentPositionParams object.
fn text_document_position_pairs(uri: &str, line: u32, character: u32) -> Vec<(String, JsonValue)> {
    vec![
        (
            "textDocument".to_string(),
            JsonValue::Object(vec![("uri".to_string(), JsonValue::Str(uri.to_string()))]),
        ),
        (
            "position".to_string(),
            JsonValue::Object(vec![
                (
                    "line".to_string(),
                    JsonValue::Number(crate::core::json::Number::Int(line as i64)),
                ),
                (
                    "character".to_string(),
                    JsonValue::Number(crate::core::json::Number::Int(character as i64)),
                ),
            ]),
        ),
    ]
}

/// Build a TextDocumentPositionParams JSON object.
fn text_document_position(uri: &str, line: u32, character: u32) -> JsonValue {
    JsonValue::Object(text_document_position_pairs(uri, line, character))
}

/// Parse a location response: null -> empty Vec, single object -> Vec with one item,
/// array -> Vec of all items.
fn parse_location_response(result: &JsonValue) -> crate::Result<Vec<Location>> {
    match result {
        JsonValue::Null => Ok(Vec::new()),
        JsonValue::Array(items) => items.iter().map(Location::from_json).collect(),
        obj @ JsonValue::Object(_) => Ok(vec![Location::from_json(obj)?]),
        _ => Err(Error::Lsp {
            server: "lsp".to_string(),
            message: "unexpected location response type".to_string(),
        }),
    }
}
