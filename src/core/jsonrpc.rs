use crate::Error;
use crate::core::json::JsonValue;

/// JSON-RPC 2.0 Request
#[derive(Debug, Clone)]
pub struct Request {
    pub id: i64,
    pub method: String,
    pub params: Option<JsonValue>,
}

impl Request {
    pub fn to_json(&self) -> JsonValue {
        let mut pairs = vec![
            ("jsonrpc".to_string(), JsonValue::Str("2.0".to_string())),
            (
                "id".to_string(),
                JsonValue::Number(crate::core::json::Number::Int(self.id)),
            ),
            ("method".to_string(), JsonValue::Str(self.method.clone())),
        ];
        if let Some(ref params) = self.params {
            pairs.push(("params".to_string(), params.clone()));
        }
        JsonValue::Object(pairs)
    }
}

/// JSON-RPC 2.0 error object
#[derive(Debug, Clone)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

/// JSON-RPC 2.0 Response
#[derive(Debug, Clone)]
pub enum Response {
    Result { id: i64, result: JsonValue },
    Error { id: i64, error: RpcError },
}

/// JSON-RPC 2.0 Notification (no id)
#[derive(Debug, Clone)]
pub struct Notification {
    pub method: String,
    pub params: Option<JsonValue>,
}

impl Notification {
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

/// Parsed JSON-RPC message
#[derive(Debug)]
pub enum Message {
    Request(Request),
    Response(Response),
    Notification(Notification),
}

impl Message {
    /// Parse a JsonValue into a JSON-RPC Message
    pub fn parse(json: &JsonValue) -> crate::Result<Message> {
        // Must be an object
        let _obj = json
            .as_object()
            .ok_or_else(|| Error::Json("JSON-RPC message must be an object".to_string()))?;

        let has_id = json.get("id").is_some();
        let has_method = json.get("method").is_some();
        let has_result = json.get("result").is_some();
        let has_error = json.get("error").is_some();

        if has_method && has_id {
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
        } else if has_method && !has_id {
            // Notification
            let method = json
                .get("method")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Json("Notification method must be a string".to_string()))?
                .to_string();
            let params = json.get("params").cloned();
            Ok(Message::Notification(Notification { method, params }))
        } else if has_result {
            // Success response
            let id = json
                .get("id")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| Error::Json("Response id must be an integer".to_string()))?;
            let result = json.get("result").cloned().unwrap_or(JsonValue::Null);
            Ok(Message::Response(Response::Result { id, result }))
        } else if has_error {
            // Error response
            let id = json
                .get("id")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| Error::Json("Response id must be an integer".to_string()))?;
            let error_obj = json
                .get("error")
                .ok_or_else(|| Error::Json("Error response missing error field".to_string()))?;
            let code = error_obj
                .get("code")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| Error::Json("Error code must be an integer".to_string()))?;
            let message = error_obj
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error")
                .to_string();
            Ok(Message::Response(Response::Error {
                id,
                error: RpcError { code, message },
            }))
        } else {
            Err(Error::Json(
                "Cannot determine JSON-RPC message type".to_string(),
            ))
        }
    }
}
