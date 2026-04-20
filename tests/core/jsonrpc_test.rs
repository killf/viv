use viv::core::json::JsonValue;
use viv::lsp::jsonrpc::*;

#[test]
fn parse_request() {
    let json = JsonValue::parse(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}"#,
    )
    .unwrap();
    let msg = Message::parse(&json).unwrap();
    match msg {
        Message::Request(req) => {
            assert_eq!(req.id, 1);
            assert_eq!(req.method, "initialize");
            assert!(req.params.is_some());
            let params = req.params.unwrap();
            assert!(params.get("capabilities").is_some());
        }
        other => panic!("expected Request, got {:?}", other),
    }
}

#[test]
fn parse_request_without_params() {
    let json = JsonValue::parse(r#"{"jsonrpc":"2.0","id":42,"method":"shutdown"}"#).unwrap();
    let msg = Message::parse(&json).unwrap();
    match msg {
        Message::Request(req) => {
            assert_eq!(req.id, 42);
            assert_eq!(req.method, "shutdown");
            assert!(req.params.is_none());
        }
        other => panic!("expected Request, got {:?}", other),
    }
}

#[test]
fn parse_result_response() {
    let json =
        JsonValue::parse(r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-03-26"}}"#)
            .unwrap();
    let msg = Message::parse(&json).unwrap();
    match msg {
        Message::Response(Response::Result { id, result }) => {
            assert_eq!(id, 1);
            assert_eq!(
                result.get("protocolVersion").unwrap().as_str().unwrap(),
                "2025-03-26"
            );
        }
        other => panic!("expected Response::Result, got {:?}", other),
    }
}

#[test]
fn parse_error_response() {
    let json = JsonValue::parse(
        r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found"}}"#,
    )
    .unwrap();
    let msg = Message::parse(&json).unwrap();
    match msg {
        Message::Response(Response::Error { id, error }) => {
            assert_eq!(id, 1);
            assert_eq!(error.code, -32601);
            assert_eq!(error.message, "Method not found");
            assert!(error.data.is_none());
        }
        other => panic!("expected Response::Error, got {:?}", other),
    }
}

#[test]
fn parse_error_response_with_data() {
    let json = JsonValue::parse(
        r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32602,"message":"Invalid params","data":"expected object"}}"#,
    )
    .unwrap();
    let msg = Message::parse(&json).unwrap();
    match msg {
        Message::Response(Response::Error { id, error }) => {
            assert_eq!(id, 2);
            assert_eq!(error.code, -32602);
            assert_eq!(error.message, "Invalid params");
            assert_eq!(error.data.unwrap().as_str().unwrap(), "expected object");
        }
        other => panic!("expected Response::Error, got {:?}", other),
    }
}

#[test]
fn parse_notification() {
    let json =
        JsonValue::parse(r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#)
            .unwrap();
    let msg = Message::parse(&json).unwrap();
    match msg {
        Message::Notification(notif) => {
            assert_eq!(notif.method, "notifications/initialized");
            assert!(notif.params.is_some());
        }
        other => panic!("expected Notification, got {:?}", other),
    }
}

#[test]
fn parse_notification_without_params() {
    let json = JsonValue::parse(r#"{"jsonrpc":"2.0","method":"notifications/cancelled"}"#).unwrap();
    let msg = Message::parse(&json).unwrap();
    match msg {
        Message::Notification(notif) => {
            assert_eq!(notif.method, "notifications/cancelled");
            assert!(notif.params.is_none());
        }
        other => panic!("expected Notification, got {:?}", other),
    }
}

#[test]
fn request_to_json_roundtrip() {
    let req = Request {
        id: 7,
        method: "tools/list".to_string(),
        params: Some(JsonValue::Object(vec![])),
    };
    let json = req.to_json();

    // Verify serialized form has jsonrpc field
    assert_eq!(json.get("jsonrpc").unwrap().as_str().unwrap(), "2.0");
    assert_eq!(json.get("id").unwrap().as_i64().unwrap(), 7);
    assert_eq!(json.get("method").unwrap().as_str().unwrap(), "tools/list");
    assert!(json.get("params").is_some());

    // Roundtrip: parse back
    let msg = Message::parse(&json).unwrap();
    match msg {
        Message::Request(parsed) => {
            assert_eq!(parsed.id, 7);
            assert_eq!(parsed.method, "tools/list");
        }
        other => panic!("expected Request, got {:?}", other),
    }
}

#[test]
fn request_to_json_without_params() {
    let req = Request {
        id: 3,
        method: "shutdown".to_string(),
        params: None,
    };
    let json = req.to_json();
    assert_eq!(json.get("jsonrpc").unwrap().as_str().unwrap(), "2.0");
    assert_eq!(json.get("id").unwrap().as_i64().unwrap(), 3);
    assert_eq!(json.get("method").unwrap().as_str().unwrap(), "shutdown");
    // params should not be present when None
    assert!(json.get("params").is_none());
}

#[test]
fn notification_to_json_roundtrip() {
    let notif = Notification {
        method: "notifications/initialized".to_string(),
        params: Some(JsonValue::Object(vec![(
            "key".to_string(),
            JsonValue::Str("value".to_string()),
        )])),
    };
    let json = notif.to_json();

    assert_eq!(json.get("jsonrpc").unwrap().as_str().unwrap(), "2.0");
    assert_eq!(
        json.get("method").unwrap().as_str().unwrap(),
        "notifications/initialized"
    );
    assert!(json.get("id").is_none()); // notifications have no id
    assert_eq!(
        json.get("params")
            .unwrap()
            .get("key")
            .unwrap()
            .as_str()
            .unwrap(),
        "value"
    );

    // Roundtrip
    let msg = Message::parse(&json).unwrap();
    match msg {
        Message::Notification(parsed) => {
            assert_eq!(parsed.method, "notifications/initialized");
        }
        other => panic!("expected Notification, got {:?}", other),
    }
}

#[test]
fn parse_invalid_missing_jsonrpc_still_works() {
    // Some implementations omit jsonrpc field; we should still parse based on structure
    let json = JsonValue::parse(r#"{"id":1,"method":"test"}"#).unwrap();
    let msg = Message::parse(&json).unwrap();
    match msg {
        Message::Request(req) => {
            assert_eq!(req.id, 1);
            assert_eq!(req.method, "test");
        }
        other => panic!("expected Request, got {:?}", other),
    }
}

#[test]
fn parse_non_object_returns_error() {
    let json = JsonValue::parse(r#"[1,2,3]"#).unwrap();
    assert!(Message::parse(&json).is_err());
}

#[test]
fn response_result_to_json() {
    let resp = Response::Result {
        id: 5,
        result: JsonValue::Object(vec![("ok".to_string(), JsonValue::Bool(true))]),
    };
    let json = resp.to_json();
    assert_eq!(json.get("jsonrpc").unwrap().as_str().unwrap(), "2.0");
    assert_eq!(json.get("id").unwrap().as_i64().unwrap(), 5);
    assert_eq!(
        json.get("result")
            .unwrap()
            .get("ok")
            .unwrap()
            .as_bool()
            .unwrap(),
        true
    );
    assert!(json.get("error").is_none());
}

#[test]
fn response_error_to_json() {
    let resp = Response::Error {
        id: 6,
        error: RpcError {
            code: -32600,
            message: "Invalid Request".to_string(),
            data: None,
        },
    };
    let json = resp.to_json();
    assert_eq!(json.get("jsonrpc").unwrap().as_str().unwrap(), "2.0");
    assert_eq!(json.get("id").unwrap().as_i64().unwrap(), 6);
    assert!(json.get("result").is_none());
    let err = json.get("error").unwrap();
    assert_eq!(err.get("code").unwrap().as_i64().unwrap(), -32600);
    assert_eq!(
        err.get("message").unwrap().as_str().unwrap(),
        "Invalid Request"
    );
}
