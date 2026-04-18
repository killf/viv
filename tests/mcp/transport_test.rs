use viv::mcp::transport::stdio::Framing;

// ── encode tests ─────────────────────────────────────────────────────────────

#[test]
fn content_length_encode() {
    let body = r#"{"jsonrpc":"2.0","method":"initialize"}"#;
    let encoded = Framing::ContentLength.encode(body);
    let expected = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
    assert_eq!(encoded, expected);
}

#[test]
fn newline_encode() {
    let body = r#"{"jsonrpc":"2.0","method":"initialize"}"#;
    let encoded = Framing::Newline.encode(body);
    assert_eq!(encoded, format!("{}\n", body));
}

// ── decode tests ─────────────────────────────────────────────────────────────

#[test]
fn content_length_decode_single() {
    let body = r#"{"id":1,"result":{}}"#;
    let frame = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
    let mut buf: Vec<u8> = frame.into_bytes();
    let result = Framing::ContentLength.try_decode(&mut buf);
    assert_eq!(result.as_deref(), Some(body));
    assert!(buf.is_empty(), "buffer should be drained after decoding");
}

#[test]
fn content_length_decode_partial_header() {
    // Header not yet complete (no \r\n\r\n yet)
    let mut buf: Vec<u8> = b"Content-Length: 20\r\n".to_vec();
    let result = Framing::ContentLength.try_decode(&mut buf);
    assert!(result.is_none());
    // Buffer must be untouched
    assert_eq!(buf, b"Content-Length: 20\r\n");
}

#[test]
fn content_length_decode_partial_body() {
    // Header complete but body is incomplete
    let body = r#"{"id":1,"result":{}}"#; // 20 bytes
    let partial_body = &body[..10];
    let frame = format!("Content-Length: {}\r\n\r\n{}", body.len(), partial_body);
    let mut buf: Vec<u8> = frame.into_bytes();
    let result = Framing::ContentLength.try_decode(&mut buf);
    assert!(result.is_none());
    // Buffer must be untouched
    assert!(buf.starts_with(b"Content-Length:"));
}

#[test]
fn newline_decode_single() {
    let body = r#"{"id":1,"result":{}}"#;
    let frame = format!("{}\n", body);
    let mut buf: Vec<u8> = frame.into_bytes();
    let result = Framing::Newline.try_decode(&mut buf);
    assert_eq!(result.as_deref(), Some(body));
    assert!(buf.is_empty(), "buffer should be drained after decoding");
}

#[test]
fn content_length_decode_leftover_bytes() {
    // Two messages concatenated — first decode should yield first, leave second
    let body1 = r#"{"id":1}"#;
    let body2 = r#"{"id":2}"#;
    let frame1 = format!("Content-Length: {}\r\n\r\n{}", body1.len(), body1);
    let frame2 = format!("Content-Length: {}\r\n\r\n{}", body2.len(), body2);
    let mut buf: Vec<u8> = [frame1.as_bytes(), frame2.as_bytes()].concat();

    let result1 = Framing::ContentLength.try_decode(&mut buf);
    assert_eq!(result1.as_deref(), Some(body1));

    let result2 = Framing::ContentLength.try_decode(&mut buf);
    assert_eq!(result2.as_deref(), Some(body2));

    assert!(buf.is_empty());
}

#[test]
fn newline_decode_leftover_bytes() {
    let line1 = r#"{"id":1}"#;
    let line2 = r#"{"id":2}"#;
    let mut buf: Vec<u8> = format!("{}\n{}\n", line1, line2).into_bytes();

    let result1 = Framing::Newline.try_decode(&mut buf);
    assert_eq!(result1.as_deref(), Some(line1));

    let result2 = Framing::Newline.try_decode(&mut buf);
    assert_eq!(result2.as_deref(), Some(line2));

    assert!(buf.is_empty());
}
