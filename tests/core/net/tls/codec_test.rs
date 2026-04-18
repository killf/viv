// Codec tests — verify TLS 1.3 message encoding and decoding.

use viv::core::net::tls::codec;

// ── ClientHello encoding ───────────────────────────────────────────

#[test]
fn encode_client_hello_has_correct_handshake_header() {
    let random = [0xAA; 32];
    let session_id = [0xBB; 32];
    let x25519_pub = [0xCC; 32];
    let mut out = Vec::new();

    codec::encode_client_hello(&random, &session_id, "example.com", &x25519_pub, &mut out);

    // First byte: handshake type = ClientHello (0x01)
    assert_eq!(out[0], codec::CLIENT_HELLO);

    // Bytes 1..4: 3-byte length of the handshake body
    let body_len = ((out[1] as usize) << 16) | ((out[2] as usize) << 8) | (out[3] as usize);
    assert_eq!(body_len, out.len() - 4);

    // legacy_version at offset 4: should be 0x0303 (TLS 1.2)
    assert_eq!(out[4], 0x03);
    assert_eq!(out[5], 0x03);

    // random at offset 6..38
    assert_eq!(&out[6..38], &[0xAA; 32]);

    // session_id_len at offset 38: should be 32
    assert_eq!(out[38], 32);

    // session_id at offset 39..71
    assert_eq!(&out[39..71], &[0xBB; 32]);

    // cipher_suites_len at offset 71..73: 0x0002
    assert_eq!(out[71], 0x00);
    assert_eq!(out[72], 0x02);

    // cipher_suite: TLS_AES_128_GCM_SHA256 = 0x1301
    assert_eq!(out[73], 0x13);
    assert_eq!(out[74], 0x01);

    // compression_methods_len at offset 75: 1
    assert_eq!(out[75], 0x01);
    // compression: null
    assert_eq!(out[76], 0x00);
}

#[test]
fn encode_client_hello_contains_required_extensions() {
    let random = [0x11; 32];
    let session_id = [0x22; 32];
    let x25519_pub = [0x33; 32];
    let mut out = Vec::new();

    codec::encode_client_hello(&random, &session_id, "test.example.org", &x25519_pub, &mut out);

    // Extensions start after the fixed fields. Find them by looking for
    // known extension type bytes in the output.

    // Server Name extension (type=0x0000) should contain the hostname
    let hostname = b"test.example.org";
    let has_hostname = out.windows(hostname.len()).any(|w| w == hostname);
    assert!(has_hostname, "ClientHello should contain the server name");

    // Key share extension should contain our public key
    let has_pubkey = out.windows(32).any(|w| w == &[0x33; 32]);
    assert!(has_pubkey, "ClientHello should contain the X25519 public key");

    // Supported versions extension should contain TLS 1.3 (0x0304)
    let has_tls13 = out.windows(2).any(|w| w == [0x03, 0x04]);
    assert!(has_tls13, "ClientHello should contain TLS 1.3 version");
}

// ── Finished encoding ──────────────────────────────────────────────

#[test]
fn encode_finished_format() {
    let verify_data = [0x42; 32];
    let mut out = Vec::new();

    codec::encode_finished(&verify_data, &mut out);

    // Handshake type: Finished (0x14)
    assert_eq!(out[0], codec::FINISHED);

    // 3-byte length = 32
    assert_eq!(out[1], 0x00);
    assert_eq!(out[2], 0x00);
    assert_eq!(out[3], 0x20);

    // verify_data
    assert_eq!(&out[4..36], &[0x42; 32]);
    assert_eq!(out.len(), 36);
}

// ── ChangeCipherSpec encoding ──────────────────────────────────────

#[test]
fn encode_change_cipher_spec_format() {
    let mut out = Vec::new();
    codec::encode_change_cipher_spec(&mut out);

    // Full TLS record: type(0x14) + version(0x0301) + length(0x0001) + payload(0x01)
    assert_eq!(out, &[0x14, 0x03, 0x01, 0x00, 0x01, 0x01]);
}

// ── ServerHello decoding ───────────────────────────────────────────

#[test]
fn decode_server_hello_with_known_bytes() {
    // Build a minimal ServerHello message by hand
    let mut msg = Vec::new();

    // Handshake header: type=ServerHello(0x02), length TBD
    msg.push(codec::SERVER_HELLO);
    let len_pos = msg.len();
    msg.push(0); msg.push(0); msg.push(0); // placeholder for length

    let body_start = msg.len();

    // legacy_version: 0x0303
    msg.push(0x03); msg.push(0x03);

    // random: 32 bytes
    let random = [0x55u8; 32];
    msg.extend_from_slice(&random);

    // session_id: length=0
    msg.push(0);

    // cipher_suite: TLS_AES_128_GCM_SHA256 = 0x1301
    msg.push(0x13); msg.push(0x01);

    // compression_method: null
    msg.push(0x00);

    // Extensions
    let ext_start = msg.len();
    msg.push(0); msg.push(0); // extensions length placeholder

    // supported_versions extension (type=43)
    msg.push(0x00); msg.push(0x2b); // type=43
    msg.push(0x00); msg.push(0x02); // length=2
    msg.push(0x03); msg.push(0x04); // TLS 1.3

    // key_share extension (type=51)
    msg.push(0x00); msg.push(0x33); // type=51
    msg.push(0x00); msg.push(0x24); // length=36
    msg.push(0x00); msg.push(0x1d); // group=x25519
    msg.push(0x00); msg.push(0x20); // key_exchange length=32
    let server_pub = [0x77u8; 32];
    msg.extend_from_slice(&server_pub);

    // Fix extensions length
    let ext_len = msg.len() - ext_start - 2;
    msg[ext_start] = (ext_len >> 8) as u8;
    msg[ext_start + 1] = ext_len as u8;

    // Fix handshake body length
    let body_len = msg.len() - body_start;
    msg[len_pos] = (body_len >> 16) as u8;
    msg[len_pos + 1] = (body_len >> 8) as u8;
    msg[len_pos + 2] = body_len as u8;

    // Decode
    let result = codec::decode_handshake(&msg).expect("decode should succeed");

    match result {
        codec::HandshakeMessage::ServerHello(sh) => {
            assert_eq!(sh.random, random);
            assert_eq!(sh.cipher_suite, 0x1301);
            assert_eq!(sh.x25519_public, server_pub);
        }
        _ => panic!("expected ServerHello"),
    }
}

#[test]
fn decode_finished_with_known_bytes() {
    let verify = [0xAB; 32];
    let mut msg = Vec::new();
    msg.push(codec::FINISHED);
    msg.push(0x00); msg.push(0x00); msg.push(0x20); // length = 32
    msg.extend_from_slice(&verify);

    let result = codec::decode_handshake(&msg).expect("decode should succeed");
    match result {
        codec::HandshakeMessage::Finished { verify_data } => {
            assert_eq!(verify_data, verify);
        }
        _ => panic!("expected Finished"),
    }
}

#[test]
fn decode_truncated_message_returns_error() {
    // Only 3 bytes — not enough for a handshake header
    let data = [0x02, 0x00, 0x00];
    assert!(codec::decode_handshake(&data).is_err());
}
