use viv::net::http::*;

#[test]
fn build_request() {
    let req = HttpRequest {
        method: "POST".into(),
        path: "/v1/messages".into(),
        headers: vec![
            ("Host".into(), "api.anthropic.com".into()),
            ("Content-Type".into(), "application/json".into()),
            ("x-api-key".into(), "test-key".into()),
        ],
        body: Some(r#"{"model":"test"}"#.into()),
    };
    let raw = req.to_bytes();
    let s = String::from_utf8(raw).unwrap();
    assert!(s.starts_with("POST /v1/messages HTTP/1.1\r\n"));
    assert!(s.contains("Host: api.anthropic.com\r\n"));
    assert!(s.contains("Content-Length: 16\r\n"));
    assert!(s.ends_with("\r\n{\"model\":\"test\"}"));
}

#[test]
fn build_request_no_body() {
    let req = HttpRequest {
        method: "GET".into(),
        path: "/".into(),
        headers: vec![("Host".into(), "example.com".into())],
        body: None,
    };
    let raw = req.to_bytes();
    let s = String::from_utf8(raw).unwrap();
    assert!(s.starts_with("GET / HTTP/1.1\r\n"));
    assert!(s.ends_with("\r\n\r\n"));
    assert!(!s.contains("Content-Length"));
}

#[test]
fn parse_response() {
    let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello";
    let resp = HttpResponse::parse(raw).unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, b"hello");
}

#[test]
fn parse_chunked_response() {
    let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n";
    let resp = HttpResponse::parse(raw).unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, b"hello world");
}

#[test]
fn get_header_case_insensitive() {
    let raw = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: 0\r\n\r\n";
    let resp = HttpResponse::parse(raw).unwrap();
    assert_eq!(resp.header("content-type"), Some("text/event-stream"));
    assert_eq!(resp.header("CONTENT-TYPE"), Some("text/event-stream"));
}

#[test]
fn parse_error_response() {
    let raw = b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 11\r\n\r\nUnauthorized";
    let resp = HttpResponse::parse(raw).unwrap();
    assert_eq!(resp.status, 401);
    assert_eq!(resp.body, b"Unauthorized");
}
