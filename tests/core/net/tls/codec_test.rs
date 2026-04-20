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

    // cipher_suites_len at offset 71..73: 0x0008 (4 suites * 2 bytes)
    assert_eq!(out[71], 0x00);
    assert_eq!(out[72], 0x08);

    // cipher_suite 1: TLS_AES_128_GCM_SHA256 = 0x1301
    assert_eq!(out[73], 0x13);
    assert_eq!(out[74], 0x01);

    // cipher_suite 2: TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256 = 0xC02F
    assert_eq!(out[75], 0xC0);
    assert_eq!(out[76], 0x2F);

    // cipher_suite 3: TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256 = 0xC02B
    assert_eq!(out[77], 0xC0);
    assert_eq!(out[78], 0x2B);

    // cipher_suite 4: TLS_EMPTY_RENEGOTIATION_INFO_SCSV = 0x00FF
    assert_eq!(out[79], 0x00);
    assert_eq!(out[80], 0xFF);

    // compression_methods_len at offset 81: 1
    assert_eq!(out[81], 0x01);
    // compression: null
    assert_eq!(out[82], 0x00);
}

#[test]
fn encode_client_hello_contains_required_extensions() {
    let random = [0x11; 32];
    let session_id = [0x22; 32];
    let x25519_pub = [0x33; 32];
    let mut out = Vec::new();

    codec::encode_client_hello(
        &random,
        &session_id,
        "test.example.org",
        &x25519_pub,
        &mut out,
    );

    // Extensions start after the fixed fields. Find them by looking for
    // known extension type bytes in the output.

    // Server Name extension (type=0x0000) should contain the hostname
    let hostname = b"test.example.org";
    let has_hostname = out.windows(hostname.len()).any(|w| w == hostname);
    assert!(has_hostname, "ClientHello should contain the server name");

    // Key share extension should contain our public key
    let has_pubkey = out.windows(32).any(|w| w == &[0x33; 32]);
    assert!(
        has_pubkey,
        "ClientHello should contain the X25519 public key"
    );

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

    // Full TLS record: type(0x14) + version(0x0303) + length(0x0001) + payload(0x01)
    assert_eq!(out, &[0x14, 0x03, 0x03, 0x00, 0x01, 0x01]);
}

// ── ServerHello decoding ───────────────────────────────────────────

#[test]
fn decode_server_hello_with_known_bytes() {
    // Build a minimal ServerHello message by hand
    let mut msg = Vec::new();

    // Handshake header: type=ServerHello(0x02), length TBD
    msg.push(codec::SERVER_HELLO);
    let len_pos = msg.len();
    msg.push(0);
    msg.push(0);
    msg.push(0); // placeholder for length

    let body_start = msg.len();

    // legacy_version: 0x0303
    msg.push(0x03);
    msg.push(0x03);

    // random: 32 bytes
    let random = [0x55u8; 32];
    msg.extend_from_slice(&random);

    // session_id: length=0
    msg.push(0);

    // cipher_suite: TLS_AES_128_GCM_SHA256 = 0x1301
    msg.push(0x13);
    msg.push(0x01);

    // compression_method: null
    msg.push(0x00);

    // Extensions
    let ext_start = msg.len();
    msg.push(0);
    msg.push(0); // extensions length placeholder

    // supported_versions extension (type=43)
    msg.push(0x00);
    msg.push(0x2b); // type=43
    msg.push(0x00);
    msg.push(0x02); // length=2
    msg.push(0x03);
    msg.push(0x04); // TLS 1.3

    // key_share extension (type=51)
    msg.push(0x00);
    msg.push(0x33); // type=51
    msg.push(0x00);
    msg.push(0x24); // length=36
    msg.push(0x00);
    msg.push(0x1d); // group=x25519
    msg.push(0x00);
    msg.push(0x20); // key_exchange length=32
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
            assert_eq!(sh.x25519_public, Some(server_pub));
        }
        _ => panic!("expected ServerHello"),
    }
}

#[test]
fn decode_finished_with_known_bytes() {
    let verify = [0xAB; 32];
    let mut msg = Vec::new();
    msg.push(codec::FINISHED);
    msg.push(0x00);
    msg.push(0x00);
    msg.push(0x20); // length = 32
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

#[test]
fn client_hello_offers_tls12_cipher_suites() {
    let random = [0u8; 32];
    let session_id = [0u8; 32];
    let x25519_pub = [0u8; 32];
    let mut out = Vec::new();
    codec::encode_client_hello(&random, &session_id, "example.com", &x25519_pub, &mut out);
    let found_c02f = out.windows(2).any(|w| w == [0xC0, 0x2F]);
    let found_c02b = out.windows(2).any(|w| w == [0xC0, 0x2B]);
    assert!(found_c02f, "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256 (0xC02F) not in ClientHello");
    assert!(found_c02b, "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256 (0xC02B) not in ClientHello");
}

#[test]
fn client_hello_supported_versions_includes_tls12() {
    let random = [0u8; 32];
    let session_id = [0u8; 32];
    let x25519_pub = [0u8; 32];
    let mut out = Vec::new();
    codec::encode_client_hello(&random, &session_id, "example.com", &x25519_pub, &mut out);
    // 0x0303 must appear in supported_versions extension
    let found_v12 = out.windows(2).any(|w| w == [0x03, 0x03]);
    assert!(found_v12, "TLS 1.2 version 0x0303 not in ClientHello");
}

#[test]
fn decode_tls12_server_hello_extracts_version() {
    // Minimal TLS 1.2 ServerHello (no extensions at all — version = legacy_version)
    let mut body = Vec::new();
    body.extend_from_slice(&[0x03, 0x03]); // legacy_version = TLS 1.2
    body.extend_from_slice(&[0x42u8; 32]); // random
    body.push(0); // session_id length = 0
    body.extend_from_slice(&[0xC0, 0x2B]); // cipher_suite
    body.push(0); // compression
    // No extensions

    let mut msg = Vec::new();
    msg.push(0x02); // SERVER_HELLO
    let len = body.len() as u32;
    msg.push((len >> 16) as u8);
    msg.push((len >> 8) as u8);
    msg.push(len as u8);
    msg.extend_from_slice(&body);

    let decoded = codec::decode_handshake(&msg).unwrap();
    if let codec::HandshakeMessage::ServerHello(sh) = decoded {
        assert_eq!(sh.version, 0x0303, "Expected TLS 1.2 version");
        assert!(sh.x25519_public.is_none(), "TLS 1.2 ServerHello has no key_share");
    } else {
        panic!("expected ServerHello");
    }
}

#[test]
fn decode_server_key_exchange_extracts_p256_pubkey() {
    use viv::core::net::tls::codec::{decode_handshake, HandshakeMessage};

    let fake_pubkey = [0x04u8; 65];
    let fake_sig = [0xABu8; 64];

    let mut body = Vec::new();
    body.push(3); // curve_type = named_curve
    body.extend_from_slice(&[0x00, 0x17]); // secp256r1
    body.push(65); // pubkey length
    body.extend_from_slice(&fake_pubkey);
    body.extend_from_slice(&[0x04, 0x01]); // rsa_pkcs1_sha256
    body.extend_from_slice(&[0x00, 64u8]); // sig length
    body.extend_from_slice(&fake_sig);

    let mut msg = vec![0x0C]; // SERVER_KEY_EXCHANGE
    let len = body.len() as u32;
    msg.push((len >> 16) as u8);
    msg.push((len >> 8) as u8);
    msg.push(len as u8);
    msg.extend_from_slice(&body);

    let decoded = decode_handshake(&msg).unwrap();
    if let HandshakeMessage::ServerKeyExchange(ske) = decoded {
        assert_eq!(ske.named_curve, 0x0017);
        assert_eq!(ske.public_key.len(), 65);
        assert_eq!(&ske.public_key, &fake_pubkey);
    } else {
        panic!("expected ServerKeyExchange");
    }
}

#[test]
fn encode_client_key_exchange_format() {
    use viv::core::net::tls::codec::encode_client_key_exchange;
    let pubkey = [0x04u8; 65];
    let mut out = Vec::new();
    encode_client_key_exchange(&pubkey, &mut out);
    assert_eq!(out[0], 0x10); // CLIENT_KEY_EXCHANGE
    assert_eq!(out[4], 65); // pubkey length byte
    assert_eq!(&out[5..70], &pubkey[..]);
}
