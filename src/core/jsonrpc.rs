use crate::Error;
use crate::core::json::{JsonValue, Number};

/// JSON-RPC 2.0 request message.
#[derive(Debug)]
pub struct Request {
    pub id: i64,
    pub method: String,
    pub params: Option<JsonValue>,
}

/// JSON-RPC 2.0 response message (success or error).
#[derive(Debug)]
pub enum Response {
    Result { id: i64, result: JsonValue },
    Error { id: i64, error: RpcError },
}

/// JSON-RPC 2.0 error object.
#[derive(Debug)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<JsonValue>,
}

/// JSON-RPC 2.0 notification (request without id).
#[derive(Debug)]
pub struct Notification {
    pub method: String,
    pub params: Option<JsonValue>,
}

/// Any JSON-RPC 2.0 message.
#[derive(Debug)]
pub enum Message {
    Request(Request),
    Response(Response),
    Notification(Notification),
}

impl Message {
    /// Parse an incoming JSON-RPC 2.0 message from a `JsonValue`.
    ///
    /// Dispatch logic:
    /// - has id + method -> Request
    /// - has id + result -> Response::Result
    /// - has id + error  -> Response::Error
    /// - has method, no id -> Notification
    pub fn parse(json: &JsonValue) -> crate::Result<Self> {
        let _obj = json
            .as_object()
            .ok_or_else(|| Error::Json("JSON-RPC message must be an object".to_string()))?;

        let has_id = json.get("id").is_some();
        let has_method = json.get("method").is_some();
        let has_result = json.get("result").is_some();
        let has_error = json.get("error").is_some();

        if has_id && has_method {
            // Request
            let id = json
                .get("id")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| Error::Json("Request id must be an integer".to_string()))?;
            let method = json
                .get("method")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Json("Request method must be a string".to_string()))?
                .to_string();
            let params = json.get("params").cloned();
            Ok(Message::Request(Request { id, method, params }))
        } else if has_id && has_result {
            // Response::Result
            let id = json
                .get("id")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| Error::Json("Response id must be an integer".to_string()))?;
            let result = json.get("result").cloned().unwrap();
            Ok(Message::Response(Response::Result { id, result }))
        } else if has_id && has_error {
            // Response::Error
            let id = json
                .get("id")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| Error::Json("Response id must be an integer".to_string()))?;
            let error = parse_rpc_error(json.get("error").unwrap())?;
            Ok(Message::Response(Response::Error { id, error }))
        } else if has_method && !has_id {
            // Notification
            let method = json
                .get("method")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Json("Notification method must be a string".to_string()))?
                .to_string();
            let params = json.get("params").cloned();
            Ok(Message::Notification(Notification { method, params }))
        } else {
            Err(Error::Json(
                "Cannot determine JSON-RPC message type".to_string(),
            ))
        }
    }
}

fn parse_rpc_error(json: &JsonValue) -> crate::Result<RpcError> {
    let code = json
        .get("code")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| Error::Json("RPC error code must be an integer".to_string()))?;
    let message = json
        .get("message")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::Json("RPC error message must be a string".to_string()))?
        .to_string();
    let data = json.get("data").cloned();
    Ok(RpcError {
        code,
        message,
        data,
    })
}

impl Request {
    /// Serialize this request to a `JsonValue` with `"jsonrpc": "2.0"`.
    pub fn to_json(&self) -> JsonValue {
        let mut pairs = vec![
            ("jsonrpc".to_string(), JsonValue::Str("2.0".to_string())),
            ("id".to_string(), JsonValue::Number(Number::Int(self.id))),
            ("method".to_string(), JsonValue::Str(self.method.clone())),
        ];
        if let Some(ref params) = self.params {
            pairs.push(("params".to_string(), params.clone()));
        }
        JsonValue::Object(pairs)
    }
}

impl Notification {
    /// Serialize this notification to a `JsonValue` with `"jsonrpc": "2.0"`.
    pub fn to_json(&self) -> JsonValue {
        let mut pairs = vec![
            ("jsonrpc".to_string(), JsonValue::Str("2.0".to_string())),
            ("method".to_string(), JsonValue::Str(self.method.clone())),
        ];
        if let Some(ref params) = self.params {
            pairs.push(("params".to_string(), params.clone()));
        }
        JsonValue::Object(pairs)
    }
}

impl Response {
    /// Serialize this response to a `JsonValue` with `"jsonrpc": "2.0"`.
    pub fn to_json(&self) -> JsonValue {
        match self {
            Response::Result { id, result } => JsonValue::Object(vec![
                ("jsonrpc".to_string(), JsonValue::Str("2.0".to_string())),
                ("id".to_string(), JsonValue::Number(Number::Int(*id))),
                ("result".to_string(), result.clone()),
            ]),
            Response::Error { id, error } => {
                let mut err_pairs = vec![
                    (
                        "code".to_string(),
                        JsonValue::Number(Number::Int(error.code)),
                    ),
                    ("message".to_string(), JsonValue::Str(error.message.clone())),
                ];
                if let Some(ref data) = error.data {
                    err_pairs.push(("data".to_string(), data.clone()));
                }
                JsonValue::Object(vec![
                    ("jsonrpc".to_string(), JsonValue::Str("2.0".to_string())),
                    ("id".to_string(), JsonValue::Number(Number::Int(*id))),
                    ("error".to_string(), JsonValue::Object(err_pairs)),
                ])
            }
        }
    }
}
