use viv::core::net::ws::*;

#[test]
fn encode_text_frame_small() {
    let frame = WsFrame::text("Hello");
    let encoded = frame.encode();

    // FIN + Text opcode = 0x81
    assert_eq!(encoded[0], 0x81);

    // MASK bit set + length 5 = 0x85
    assert_eq!(encoded[1], 0x85);

    // 2 bytes header + 4 bytes mask + 5 bytes payload = 11 total
    assert_eq!(encoded.len(), 11);

    // Verify we can unmask the payload to recover original text
    let mask = &encoded[2..6];
    let masked_payload = &encoded[6..];
    let mut unmasked = Vec::new();
    for (i, &b) in masked_payload.iter().enumerate() {
        unmasked.push(b ^ mask[i % 4]);
    }
    assert_eq!(&unmasked, b"Hello");
}

#[test]
fn encode_text_frame_medium() {
    // 200-byte payload triggers 16-bit extended length
    let payload = "x".repeat(200);
    let frame = WsFrame::text(&payload);
    let encoded = frame.encode();

    // FIN + Text opcode = 0x81
    assert_eq!(encoded[0], 0x81);

    // MASK bit + 126 (extended 16-bit length marker) = 0xFE
    assert_eq!(encoded[1], 0xFE);

    // Next 2 bytes are big-endian length = 200
    let ext_len = u16::from_be_bytes([encoded[2], encoded[3]]);
    assert_eq!(ext_len, 200);

    // 2 bytes header + 2 bytes extended length + 4 bytes mask + 200 bytes payload = 208
    assert_eq!(encoded.len(), 208);
}

#[test]
fn decode_unmasked_text_frame() {
    // Server sends unmasked text frame: "Hello"
    let data = [0x81, 0x05, b'H', b'e', b'l', b'l', b'o'];
    let result = WsFrame::decode(&data).unwrap();
    assert!(result.is_some());

    let (frame, consumed) = result.unwrap();
    assert_eq!(frame.opcode, WsOpcode::Text);
    assert_eq!(frame.payload, b"Hello");
    assert_eq!(consumed, 7);
}

#[test]
fn decode_incomplete_returns_none() {
    // Only 1 byte — not enough for even the minimal header
    let data = [0x81];
    let result = WsFrame::decode(&data).unwrap();
    assert!(result.is_none());

    // Header says 5 bytes payload but only 2 present
    let data = [0x81, 0x05, b'H', b'e'];
    let result = WsFrame::decode(&data).unwrap();
    assert!(result.is_none());
}

#[test]
fn close_frame() {
    let frame = WsFrame::close();
    assert_eq!(frame.opcode, WsOpcode::Close);
    assert!(frame.payload.is_empty());

    let encoded = frame.encode();
    // FIN + Close opcode = 0x88
    assert_eq!(encoded[0], 0x88);
    // MASK bit + length 0 = 0x80
    assert_eq!(encoded[1], 0x80);
    // 2 bytes header + 4 bytes mask + 0 bytes payload = 6
    assert_eq!(encoded.len(), 6);
}

#[test]
fn pong_frame() {
    let data = b"ping-data";
    let frame = WsFrame::pong(data);
    assert_eq!(frame.opcode, WsOpcode::Pong);
    assert_eq!(frame.payload, data);

    let encoded = frame.encode();
    // FIN + Pong opcode = 0x8A
    assert_eq!(encoded[0], 0x8A);

    // Verify payload is recoverable
    let mask_start = 2;
    let mask = &encoded[mask_start..mask_start + 4];
    let masked_payload = &encoded[mask_start + 4..];
    let mut unmasked = Vec::new();
    for (i, &b) in masked_payload.iter().enumerate() {
        unmasked.push(b ^ mask[i % 4]);
    }
    assert_eq!(&unmasked, data);
}

#[test]
fn decode_masked_frame() {
    // Server frame with mask (unusual but spec-legal)
    let payload = b"Hi";
    let mask: [u8; 4] = [0x12, 0x34, 0x56, 0x78];
    let masked: Vec<u8> = payload
        .iter()
        .enumerate()
        .map(|(i, &b)| b ^ mask[i % 4])
        .collect();

    let mut data = vec![0x81u8]; // FIN + Text
    data.push(0x82); // MASK bit + length 2
    data.extend_from_slice(&mask);
    data.extend_from_slice(&masked);

    let result = WsFrame::decode(&data).unwrap();
    assert!(result.is_some());
    let (frame, consumed) = result.unwrap();
    assert_eq!(frame.opcode, WsOpcode::Text);
    assert_eq!(frame.payload, b"Hi");
    assert_eq!(consumed, data.len());
}

#[test]
fn decode_extended_16bit_length() {
    // Server frame with 200-byte payload using 16-bit extended length
    let payload = vec![0x41u8; 200]; // 200 bytes of 'A'
    let mut data = vec![0x81u8]; // FIN + Text
    data.push(126); // extended 16-bit length marker (no mask)
    data.extend_from_slice(&200u16.to_be_bytes());
    data.extend_from_slice(&payload);

    let result = WsFrame::decode(&data).unwrap();
    assert!(result.is_some());
    let (frame, consumed) = result.unwrap();
    assert_eq!(frame.opcode, WsOpcode::Text);
    assert_eq!(frame.payload.len(), 200);
    assert_eq!(consumed, data.len());
}

#[test]
fn build_upgrade_request_contains_required_headers() {
    let req = build_upgrade_request("example.com", "/ws");
    let s = String::from_utf8(req).unwrap();

    assert!(s.starts_with("GET /ws HTTP/1.1\r\n"));
    assert!(s.contains("Host: example.com\r\n"));
    assert!(s.contains("Upgrade: websocket\r\n"));
    assert!(s.contains("Connection: Upgrade\r\n"));
    assert!(s.contains("Sec-WebSocket-Version: 13\r\n"));
    assert!(s.contains("Sec-WebSocket-Key: "));
    assert!(s.ends_with("\r\n\r\n"));
}

#[test]
fn base64_encode_rfc_vectors() {
    // RFC 4648 test vectors
    assert_eq!(base64_encode(b""), "");
    assert_eq!(base64_encode(b"f"), "Zg==");
    assert_eq!(base64_encode(b"fo"), "Zm8=");
    assert_eq!(base64_encode(b"foo"), "Zm9v");
    assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
    assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
    assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
}
