// TLS stream integration tests — verify TlsStream connect/read/write paths.

use viv::core::net::tls::codec;
use viv::core::net::tls::record::RecordLayer;

// ── TlsStream connect error paths ─────────────────────────────────

#[test]
fn tls_connect_to_unreachable_host_fails() {
    use viv::core::net::tls::TlsStream;
    // 10.255.255.1 is a non-routable test address; this should fail at the
    // network level (not DNS) since the IP is valid but unreachable.
    let result = TlsStream::connect("10.255.255.1", 443);
    assert!(result.is_err(), "connect to unreachable host should fail");
}

#[test]
fn tls_connect_to_connection_refused_fails() {
    use viv::core::net::tls::TlsStream;
    // Connect to localhost port where nothing is listening — should get connection refused
    let result = TlsStream::connect("127.0.0.1", 59999);
    assert!(result.is_err(), "connect to closed port should fail");
}

// ── RecordLayer encrypted record size limits ──────────────────────

#[test]
fn encrypted_record_header_and_trailer_sizes() {
    let key = [0x01; 16];
    let iv = [0x02; 12];

    let mut writer = RecordLayer::new();
    writer.install_encrypter(key, iv);

    // Encrypt minimal payload (1 byte)
    let mut out1 = Vec::new();
    writer.write_encrypted(codec::APPLICATION_DATA, b"x", &mut out1);

    // Outer header = 5 bytes, ciphertext = payload(1) + type_pad(1) + tag(16) = 18
    assert_eq!(out1.len(), 5 + 18);
    assert_eq!(out1[0], codec::APPLICATION_DATA);
    assert_eq!(out1[3], 0x00);
    assert_eq!(out1[4], 18); // length = 18
}

#[test]
fn encrypt_empty_payload_produces_valid_record() {
    let key = [0xCC; 16];
    let iv = [0xDD; 12];

    let mut writer = RecordLayer::new();
    writer.install_encrypter(key, iv);

    let mut reader = RecordLayer::new();
    reader.install_decrypter(key, iv);

    // Encrypt empty payload — inner = "" + content_type = 1 byte
    let mut encrypted = Vec::new();
    writer.write_encrypted(codec::APPLICATION_DATA, &[], &mut encrypted);

    // Decrypt should succeed
    let (ct, plaintext, _) = reader.read_record(&encrypted).expect("decrypt empty");
    assert_eq!(ct, codec::APPLICATION_DATA);
    assert!(plaintext.is_empty());
}

#[test]
fn decrypt_record_with_wrong_key_fails() {
    let key = [0x01; 16];
    let iv = [0x02; 12];

    let mut writer = RecordLayer::new();
    writer.install_encrypter(key, iv);

    let mut reader = RecordLayer::new();
    // Wrong key for decryption
    reader.install_decrypter([0xFF; 16], iv);

    let payload = b"secret data";
    let mut encrypted = Vec::new();
    writer.write_encrypted(codec::APPLICATION_DATA, payload, &mut encrypted);

    // Decryption with wrong key should fail
    let result = reader.read_record(&encrypted);
    assert!(
        result.is_err(),
        "decrypt with wrong key should fail"
    );
}

#[test]
fn decrypt_record_with_wrong_iv_fails() {
    let key = [0x01; 16];
    let iv = [0x02; 12];

    let mut writer = RecordLayer::new();
    writer.install_encrypter(key, iv);

    let mut reader = RecordLayer::new();
    // Correct key, wrong IV
    reader.install_decrypter(key, [0xFF; 12]);

    let payload = b"more secret";
    let mut encrypted = Vec::new();
    writer.write_encrypted(codec::APPLICATION_DATA, payload, &mut encrypted);

    let result = reader.read_record(&encrypted);
    assert!(
        result.is_err(),
        "decrypt with wrong IV should fail"
    );
}

#[test]
fn decrypt_record_with_tampered_ciphertext_fails() {
    let key = [0xAB; 16];
    let iv = [0xCD; 12];

    let mut writer = RecordLayer::new();
    writer.install_encrypter(key, iv);

    let mut reader = RecordLayer::new();
    reader.install_decrypter(key, iv);

    let payload = b"integrity check";
    let mut encrypted = Vec::new();
    writer.write_encrypted(codec::APPLICATION_DATA, payload, &mut encrypted);

    // Tamper with the ciphertext (after the 5-byte header)
    if encrypted.len() > 10 {
        encrypted[6] ^= 0x42; // Flip a bit in the ciphertext
    }

    let result = reader.read_record(&encrypted);
    assert!(
        result.is_err(),
        "tampered ciphertext should fail authentication"
    );
}

#[test]
fn decrypt_record_with_truncated_body_fails() {
    let key = [0x11; 16];
    let iv = [0x22; 12];

    let mut writer = RecordLayer::new();
    writer.install_encrypter(key, iv);

    let mut reader = RecordLayer::new();
    reader.install_decrypter(key, iv);

    let payload = b"full record";
    let mut encrypted = Vec::new();
    writer.write_encrypted(codec::APPLICATION_DATA, payload, &mut encrypted);

    // Truncate the record — remove last byte
    encrypted.truncate(encrypted.len() - 1);

    let result = reader.read_record(&encrypted);
    assert!(
        result.is_err(),
        "truncated encrypted record should fail"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("truncated") || err.contains("too short"),
        "error should mention truncated: {}",
        err
    );
}

// ── Handshake message spanning records (codec boundary) ────────────

#[test]
fn decode_handshake_multiple_messages_in_one_record() {
    // Simulate: ServerHello + EncryptedExtensions in one record payload
    use viv::core::net::tls::codec::decode_handshake;

    // Build a ServerHello body (minimal valid)
    let mut sh_body = Vec::new();
    sh_body.push(0x03); // legacy_version
    sh_body.push(0x03);
    sh_body.extend_from_slice(&[0x55; 32]); // random
    sh_body.push(0); // session_id empty
    sh_body.push(0x13); // cipher_suite TLS_AES_128_GCM_SHA256
    sh_body.push(0x01);
    sh_body.push(0x00); // compression

    // Extensions: supported_versions + key_share
    let mut exts = Vec::new();
    exts.push(0x00); // supported_versions type
    exts.push(0x2b);
    exts.push(0x00);
    exts.push(0x02);
    exts.push(0x03); // TLS 1.3
    exts.push(0x04);
    exts.push(0x00); // key_share type
    exts.push(0x33);
    exts.push(0x00);
    exts.push(0x24);
    exts.push(0x00);
    exts.push(0x1d);
    exts.push(0x00);
    exts.push(0x20);
    exts.extend_from_slice(&[0x77; 32]); // server pubkey

    sh_body.extend_from_slice(&(exts.len() as u16).to_be_bytes());
    sh_body.extend_from_slice(&exts);

    // Wrap in handshake header
    let mut sh_msg = Vec::new();
    sh_msg.push(codec::SERVER_HELLO);
    sh_msg.extend_from_slice(&(sh_body.len() as u32).to_be_bytes()[1..]);
    sh_msg.extend_from_slice(&sh_body);

    let sh_msg_len = sh_msg.len(); // save length before moving

    // Now add EncryptedExtensions message
    let mut ee_msg = Vec::new();
    ee_msg.push(codec::ENCRYPTED_EXTENSIONS);
    ee_msg.extend_from_slice(&[0u8, 0, 0]); // length = 0

    // Combine in one record payload
    let mut combined = sh_msg;
    combined.extend_from_slice(&ee_msg);

    // Decode first message (ServerHello)
    let msg1 = decode_handshake(&combined).expect("should decode ServerHello");
    assert!(matches!(msg1, viv::core::net::tls::codec::HandshakeMessage::ServerHello(_)));

    // Decode second message (EncryptedExtensions)
    let msg2 = decode_handshake(&combined[sh_msg_len..]).expect("should decode EncryptedExtensions");
    assert!(matches!(msg2, viv::core::net::tls::codec::HandshakeMessage::EncryptedExtensions));
}

// ── Codec: Finished decode with various lengths ───────────────────

#[test]
fn decode_finished_exactly_32_bytes() {
    use viv::core::net::tls::codec::decode_handshake;

    let mut msg = Vec::new();
    msg.push(codec::FINISHED);
    msg.extend_from_slice(&0x00_0020_u32.to_be_bytes()[1..]); // length = 32
    msg.extend_from_slice(&[0x42; 32]);

    let result = decode_handshake(&msg).expect("should decode");
    match result {
        viv::core::net::tls::codec::HandshakeMessage::Finished { verify_data } => {
            assert_eq!(verify_data, [0x42; 32]);
        }
        _ => panic!("expected Finished message"),
    }
}

#[test]
fn decode_finished_more_than_32_bytes_ignores_extra() {
    use viv::core::net::tls::codec::decode_handshake;

    let mut msg = Vec::new();
    msg.push(codec::FINISHED);
    msg.extend_from_slice(&0x00_0028_u32.to_be_bytes()[1..]); // length = 40
    msg.extend_from_slice(&[0x99; 32]); // verify_data
    msg.extend_from_slice(&[0xAA; 8]); // extra bytes

    let result = decode_handshake(&msg).expect("should decode");
    match result {
        viv::core::net::tls::codec::HandshakeMessage::Finished { verify_data } => {
            assert_eq!(verify_data, [0x99; 32]);
        }
        _ => panic!("expected Finished message"),
    }
}

// ── Codec: Certificate decode ─────────────────────────────────────

#[test]
fn decode_certificate_empty_chain() {
    use viv::core::net::tls::codec::decode_handshake;

    // Certificate with empty certificate list
    let mut msg = Vec::new();
    msg.push(codec::CERTIFICATE);
    // Handshake header: type(1) + length(3)
    // body = request_context(1) + certificate_list(3+0=3) = 4 bytes
    let body_len: u32 = 4;
    let len_bytes = body_len.to_be_bytes();
    msg.extend_from_slice(&[len_bytes[1], len_bytes[2], len_bytes[3]]);

    // request_context length = 0
    msg.push(0);
    // certificate_list length = 0 (empty list)
    msg.extend_from_slice(&[0, 0, 0]);

    let result = decode_handshake(&msg).expect("should decode empty certificate");
    match result {
        viv::core::net::tls::codec::HandshakeMessage::Certificate(certs) => {
            assert!(certs.is_empty(), "expected empty cert chain");
        }
        _ => panic!("expected Certificate message"),
    }
}

// ── Record layer: plaintext write sizes ─────────────────────────

#[test]
fn write_plaintext_respects_16k_limit() {
    let record = RecordLayer::new();

    // Write exactly 16384 bytes (max plaintext record size)
    let large_payload = vec![0x42u8; 16384];
    let mut out = Vec::new();
    record.write_plaintext(codec::APPLICATION_DATA, &large_payload, &mut out);

    // Header = 5 bytes, payload = 16384
    assert_eq!(out.len(), 5 + 16384);
    assert_eq!(out[3], 0x40); // 16384 >> 8
    assert_eq!(out[4], 0x00); // 16384 & 0xFF
}

#[test]
fn write_plaintext_one_byte_payload() {
    let record = RecordLayer::new();
    let mut out = Vec::new();
    record.write_plaintext(codec::HANDSHAKE, b"x", &mut out);

    assert_eq!(out.len(), 6);
    assert_eq!(out[3], 0x00);
    assert_eq!(out[4], 0x01); // length = 1
}
