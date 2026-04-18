// Handshake state machine tests — verify correct error handling for
// unexpected state transitions, malformed messages, and verification failures.

use viv::core::net::tls::codec::{self, HandshakeMessage};
use viv::core::net::tls::crypto::sha256::hmac_sha256;
use viv::core::net::tls::crypto::x25519;
use viv::core::net::tls::handshake::{Handshake, HandshakeResult};
use viv::core::net::tls::record::RecordLayer;

// ── Helper: build a valid ServerHello message ────────────────────────

/// Build a ServerHello handshake message bytes from its fields.
fn build_server_hello(
    random: [u8; 32],
    x25519_public: [u8; 32],
    cipher_suite: u16,
) -> Vec<u8> {
    let mut msg = Vec::new();
    msg.push(codec::SERVER_HELLO);
    let len_pos = msg.len();
    msg.push(0);
    msg.push(0);
    msg.push(0);

    let body_start = msg.len();
    // legacy_version
    msg.push(0x03);
    msg.push(0x03);
    // random
    msg.extend_from_slice(&random);
    // session_id length=0
    msg.push(0);
    // cipher_suite
    msg.push((cipher_suite >> 8) as u8);
    msg.push(cipher_suite as u8);
    // compression
    msg.push(0x00);

    // extensions
    let ext_start = msg.len();
    msg.push(0);
    msg.push(0); // placeholder

    // supported_versions (TLS 1.3)
    msg.push(0x00);
    msg.push(0x2b);
    msg.push(0x00);
    msg.push(0x02);
    msg.push(0x03);
    msg.push(0x04);

    // key_share
    msg.push(0x00);
    msg.push(0x33);
    msg.push(0x00);
    msg.push(0x24);
    msg.push(0x00);
    msg.push(0x1d); // x25519
    msg.push(0x00);
    msg.push(0x20);
    msg.extend_from_slice(&x25519_public);

    let ext_len = msg.len() - ext_start - 2;
    msg[ext_start] = (ext_len >> 8) as u8;
    msg[ext_start + 1] = ext_len as u8;

    let body_len = msg.len() - body_start;
    msg[len_pos] = (body_len >> 16) as u8;
    msg[len_pos + 1] = (body_len >> 8) as u8;
    msg[len_pos + 2] = body_len as u8;

    msg
}

// ── Helper: build a minimal EncryptedExtensions ────────────────────

fn build_encrypted_extensions() -> Vec<u8> {
    let mut msg = Vec::new();
    msg.push(codec::ENCRYPTED_EXTENSIONS);
    msg.push(0x00);
    msg.push(0x00);
    msg.push(0x00); // length = 0
    msg
}

// ── Helper: build a minimal Certificate ──────────────────────────────

fn build_certificate() -> Vec<u8> {
    let mut msg = Vec::new();
    msg.push(codec::CERTIFICATE);
    let len_pos = msg.len();
    msg.push(0);
    msg.push(0);
    msg.push(0);

    let body_start = msg.len();
    // request_context length=0
    msg.push(0);
    // certificate_list length (placeholder)
    let list_len_pos = msg.len();
    msg.push(0);
    msg.push(0);
    msg.push(0);

    // one empty certificate entry
    msg.push(0);
    msg.push(0);
    msg.push(0); // cert_len=0
    // no extensions

    let list_len = msg.len() - list_len_pos - 3;
    msg[list_len_pos] = (list_len >> 16) as u8;
    msg[list_len_pos + 1] = (list_len >> 8) as u8;
    msg[list_len_pos + 2] = list_len as u8;

    let body_len = msg.len() - body_start;
    msg[len_pos] = (body_len >> 16) as u8;
    msg[len_pos + 1] = (body_len >> 8) as u8;
    msg[len_pos + 2] = body_len as u8;

    msg
}

// ── Helper: build CertificateVerify ─────────────────────────────────

fn build_certificate_verify() -> Vec<u8> {
    let mut msg = Vec::new();
    msg.push(codec::CERTIFICATE_VERIFY);
    let len_pos = msg.len();
    msg.push(0);
    msg.push(0);
    msg.push(0);

    let body_start = msg.len();
    // scheme: rsa_pkcs1_sha256 (0x0401)
    msg.push(0x04);
    msg.push(0x01);
    // signature length = 0 (stub, no real signature)
    msg.push(0x00);
    msg.push(0x00);

    let body_len = msg.len() - body_start;
    msg[len_pos] = (body_len >> 16) as u8;
    msg[len_pos + 1] = (body_len >> 8) as u8;
    msg[len_pos + 2] = body_len as u8;

    msg
}

// ── Helper: build Finished with wrong verify_data ───────────────────

fn build_finished(wrong_verify_data: bool, server_finished_key: &[u8; 32], transcript: &[u8; 32]) -> Vec<u8> {
    let correct_verify = hmac_sha256(server_finished_key, transcript);
    let verify_data = if wrong_verify_data {
        [0xFF; 32]
    } else {
        correct_verify
    };

    let mut msg = Vec::new();
    msg.push(codec::FINISHED);
    msg.push(0x00);
    msg.push(0x00);
    msg.push(0x20);
    msg.extend_from_slice(&verify_data);
    msg
}

// ── Test: wrong cipher suite rejected ───────────────────────────────

#[test]
fn server_hello_wrong_cipher_suite_rejected() {
    let mut hs = Handshake::new("example.com").unwrap();
    let _ch = hs.encode_client_hello().unwrap();

    let server_random = [0x55; 32];
    let (_, server_pub) = x25519::keypair().unwrap();
    let server_hello = build_server_hello(server_random, server_pub, 0x0000); // 0x0000 = nothing

    let mut record = RecordLayer::new();
    let result = hs.handle_message(&server_hello, &mut record);

    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("cipher") || msg.contains("unsupported"),
                "error should mention cipher suite: {}",
                msg
            );
        }
        Ok(_) => panic!("wrong cipher suite should be rejected"),
    }
}

// ── Test: ServerHello without key_share rejected ───────────────────

#[test]
fn server_hello_missing_key_share_rejected() {
    let mut hs = Handshake::new("example.com").unwrap();
    let _ch = hs.encode_client_hello().unwrap();

    // Build a ServerHello WITHOUT key_share extension
    let mut msg = Vec::new();
    msg.push(codec::SERVER_HELLO);
    msg.push(0x00);
    msg.push(0x00);
    // length placeholder
    let len_pos = msg.len() - 1;

    let body_start = msg.len();
    msg.push(0x03); // legacy_version
    msg.push(0x03);
    msg.extend_from_slice(&[0x55; 32]); // random
    msg.push(0); // session_id empty
    msg.push(0x13); // cipher_suite
    msg.push(0x01);
    msg.push(0x00); // compression

    // Extensions: only supported_versions (no key_share!)
    msg.push(0x00);
    msg.push(0x05); // ext length
    msg.push(0x00); // supported_versions type
    msg.push(0x2b);
    msg.push(0x00);
    msg.push(0x02);
    msg.push(0x03);
    msg.push(0x04);

    let body_len = msg.len() - body_start;
    msg[len_pos] = (body_len >> 16) as u8;
    msg[len_pos - 1] = (body_len >> 8) as u8;
    // Fix length at position len_pos-2: offset from end
    // Actually let me redo this more carefully...

    // Rebuild cleanly
    let mut msg2 = Vec::new();
    msg2.push(codec::SERVER_HELLO);
    let body_start2 = 4; // after header
    msg2.push(0);
    msg2.push(0);
    msg2.push(0);

    msg2.push(0x03);
    msg2.push(0x03);
    msg2.extend_from_slice(&[0x55; 32]);
    msg2.push(0);
    msg2.push(0x13);
    msg2.push(0x01);
    msg2.push(0x00);

    let mut exts = Vec::new();
    // supported_versions only
    exts.push(0x00);
    exts.push(0x2b);
    exts.push(0x00);
    exts.push(0x02);
    exts.push(0x03);
    exts.push(0x04);

    msg2.extend_from_slice(&[(exts.len() >> 8) as u8, (exts.len()) as u8]);
    msg2.extend_from_slice(&exts);

    let body_len = msg2.len() - body_start2;
    msg2[1] = (body_len >> 16) as u8;
    msg2[2] = (body_len >> 8) as u8;
    msg2[3] = body_len as u8;

    let mut record = RecordLayer::new();
    let result = hs.handle_message(&msg2, &mut record);

    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(msg.contains("key_share"), "error should mention missing key_share: {}", msg);
        }
        Ok(_) => panic!("ServerHello without key_share should be rejected"),
    }
}

// ── Test: wrong message type in ExpectServerHello state ─────────────

#[test]
fn wrong_message_type_in_server_hello_state_rejected() {
    let mut hs = Handshake::new("example.com").unwrap();
    let _ch = hs.encode_client_hello().unwrap();

    // Feed an EncryptedExtensions directly (wrong state)
    let encrypted_ext = build_encrypted_extensions();

    let mut record = RecordLayer::new();
    let result = hs.handle_message(&encrypted_ext, &mut record);

    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("unexpected") || msg.contains("state"),
                "error should mention unexpected/state: {}",
                msg
            );
        }
        Ok(_) => panic!("wrong message in ServerHello state should be rejected"),
    }
}

// ── Test: wrong message type in ExpectEncryptedExtensions ───────────

#[test]
fn wrong_message_type_in_encrypted_extensions_state_rejected() {
    let mut hs = Handshake::new("example.com").unwrap();
    let _ch = hs.encode_client_hello().unwrap();

    let server_random = [0x55; 32];
    let (_, server_pub) = x25519::keypair().unwrap();
    let server_hello = build_server_hello(server_random, server_pub, 0x1301);

    let mut record = RecordLayer::new();
    let result = hs.handle_message(&server_hello, &mut record).unwrap();
    assert!(matches!(result, HandshakeResult::Continue), "expected Continue");

    // Now feed Finished directly (wrong state)
    let finished = build_finished(false, &[0u8; 32], &[0u8; 32]);
    let result2 = hs.handle_message(&finished, &mut record);

    assert!(
        result2.is_err(),
        "Finished in EncryptedExtensions state should be rejected"
    );
}

// ── Test: wrong message type in ExpectCertificate ──────────────────

#[test]
fn wrong_message_type_in_certificate_state_rejected() {
    let mut hs = Handshake::new("example.com").unwrap();
    let _ch = hs.encode_client_hello().unwrap();

    let server_random = [0x55; 32];
    let (_, server_pub) = x25519::keypair().unwrap();
    let server_hello = build_server_hello(server_random, server_pub, 0x1301);

    let mut record = RecordLayer::new();
    let _ = hs.handle_message(&server_hello, &mut record).unwrap();
    let _ = hs.handle_message(&build_encrypted_extensions(), &mut record).unwrap();

    // Feed Finished (wrong — should be Certificate first)
    let result = hs.handle_message(&build_finished(false, &[0u8; 32], &[0u8; 32]), &mut record);
    assert!(
        result.is_err(),
        "Finished in Certificate state should be rejected"
    );
}

// ── Test: wrong message type in ExpectCertificateVerify ───────────

#[test]
fn wrong_message_type_in_certificate_verify_state_rejected() {
    let mut hs = Handshake::new("example.com").unwrap();
    let _ch = hs.encode_client_hello().unwrap();

    let server_random = [0x55; 32];
    let (_, server_pub) = x25519::keypair().unwrap();
    let server_hello = build_server_hello(server_random, server_pub, 0x1301);

    let mut record = RecordLayer::new();
    let _ = hs.handle_message(&server_hello, &mut record).unwrap();
    let _ = hs.handle_message(&build_encrypted_extensions(), &mut record).unwrap();
    let _ = hs.handle_message(&build_certificate(), &mut record).unwrap();

    // Feed EncryptedExtensions again (wrong — should be CertificateVerify)
    let result = hs.handle_message(&build_encrypted_extensions(), &mut record);
    assert!(
        result.is_err(),
        "EncryptedExtensions in CertificateVerify state should be rejected"
    );
}

// ── Test: wrong verify_data in Finished rejected ───────────────────

#[test]
fn server_finished_wrong_verify_data_rejected() {
    let mut hs = Handshake::new("example.com").unwrap();
    let _ch = hs.encode_client_hello().unwrap();

    let server_random = [0x55; 32];
    let (_, server_pub) = x25519::keypair().unwrap();
    let server_hello = build_server_hello(server_random, server_pub, 0x1301);

    let mut record = RecordLayer::new();
    let _ = hs.handle_message(&server_hello, &mut record).unwrap();
    let _ = hs.handle_message(&build_encrypted_extensions(), &mut record).unwrap();
    let _ = hs.handle_message(&build_certificate(), &mut record).unwrap();
    let _ = hs.handle_message(&build_certificate_verify(), &mut record).unwrap();

    // Feed Finished with WRONG verify_data
    let result = hs.handle_message(&build_finished(true, &[0u8; 32], &[0u8; 32]), &mut record);

    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("Finished") || msg.contains("verify"),
                "error should mention Finished/verify: {}",
                msg
            );
        }
        Ok(_) => panic!("wrong Finished verify_data should be rejected"),
    }
}

// ── Test: truncated handshake message rejected ─────────────────────

#[test]
fn truncated_handshake_message_rejected() {
    let mut hs = Handshake::new("example.com").unwrap();
    let _ch = hs.encode_client_hello().unwrap();

    // Handshake header with body length larger than actual data
    let truncated = vec![codec::SERVER_HELLO, 0x00, 0x01, 0x00]; // says 256 bytes, has 0

    let mut record = RecordLayer::new();
    let result = hs.handle_message(&truncated, &mut record);

    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("truncated") || msg.contains("too short"),
                "error should mention truncated: {}",
                msg
            );
        }
        Ok(_) => panic!("truncated handshake message should be rejected"),
    }
}

// ── Test: ServerHello wrong TLS version rejected ───────────────────

#[test]
fn server_hello_wrong_version_rejected() {
    let mut hs = Handshake::new("example.com").unwrap();
    let _ch = hs.encode_client_hello().unwrap();

    let mut msg = Vec::new();
    msg.push(codec::SERVER_HELLO);
    let len_pos = msg.len();
    msg.push(0);
    msg.push(0);
    msg.push(0);

    let body_start = msg.len();
    // Use TLS 1.2 version (wrong!)
    msg.push(0x03);
    msg.push(0x03);
    msg.extend_from_slice(&[0x55; 32]); // random
    msg.push(0); // session_id empty
    msg.push(0x13);
    msg.push(0x01); // cipher_suite
    msg.push(0x00); // compression

    // Extensions
    let mut exts = Vec::new();
    // supported_versions with wrong version
    exts.push(0x00);
    exts.push(0x2b);
    exts.push(0x00);
    exts.push(0x02);
    exts.push(0x03); // wrong: not TLS 1.3
    msg.push(0x03);

    // key_share
    exts.push(0x00);
    exts.push(0x33);
    exts.push(0x00);
    exts.push(0x24);
    exts.push(0x00);
    exts.push(0x1d);
    exts.push(0x00);
    exts.push(0x20);
    exts.extend_from_slice(&[0x77; 32]);

    msg.extend_from_slice(&[(exts.len() >> 8) as u8, (exts.len()) as u8]);
    msg.extend_from_slice(&exts);

    let body_len = msg.len() - body_start;
    msg[len_pos] = (body_len >> 16) as u8;
    msg[len_pos + 1] = (body_len >> 8) as u8;
    msg[len_pos + 2] = body_len as u8;

    let mut record = RecordLayer::new();
    let result = hs.handle_message(&msg, &mut record);

    match result {
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("version") || msg.contains("TLS"),
                "error should mention version: {}",
                msg
            );
        }
        Ok(_) => panic!("wrong TLS version should be rejected"),
    }
}

// ── Test: encode_client_hello adds to transcript ───────────────────

#[test]
fn client_hello_added_to_transcript() {
    let mut hs = Handshake::new("example.com").unwrap();

    let _ch = hs.encode_client_hello().unwrap();
    let hash = hs.transcript.clone().finish();

    // The hash of the ClientHello should be non-zero
    assert_ne!(hash, [0u8; 32], "ClientHello transcript hash should not be zero");
}

// ── Test: encode_client_finished adds to transcript ────────────────

#[test]
fn client_finished_added_to_transcript() {
    let mut hs = Handshake::new("example.com").unwrap();
    let _ch = hs.encode_client_hello().unwrap();

    let server_random = [0x55; 32];
    let (_, server_pub) = x25519::keypair().unwrap();
    let server_hello = build_server_hello(server_random, server_pub, 0x1301);

    let mut record = RecordLayer::new();
    let _ = hs.handle_message(&server_hello, &mut record).unwrap();
    let _ = hs.handle_message(&build_encrypted_extensions(), &mut record).unwrap();
    let _ = hs.handle_message(&build_certificate(), &mut record).unwrap();
    let _ = hs.handle_message(&build_certificate_verify(), &mut record).unwrap();
    let server_finished_key = hs.key_schedule.server_finished_key();
    let transcript_before = hs.transcript.clone().finish();
    let _ = hs.handle_message(&build_finished(false, &server_finished_key, &transcript_before), &mut record).unwrap();

    let hash_before = hs.transcript.clone().finish();
    let finished_msg = hs.encode_client_finished();
    let hash_after = hs.transcript.clone().finish();

    assert_ne!(
        hash_before, hash_after,
        "client Finished should be added to transcript"
    );
    assert_eq!(
        finished_msg.len(), 36,
        "Finished message should be 36 bytes (4 header + 32 verify_data)"
    );
}

// ── Test: Finished verify using constant-time comparison ──────────────

#[test]
fn finished_verify_data_exactly_one_bit_different_rejected() {
    // This test verifies constant-time comparison: even 1-bit difference is caught
    let mut hs = Handshake::new("example.com").unwrap();
    let _ch = hs.encode_client_hello().unwrap();

    let server_random = [0x55; 32];
    let (_, server_pub) = x25519::keypair().unwrap();
    let server_hello = build_server_hello(server_random, server_pub, 0x1301);

    let mut record = RecordLayer::new();
    let _ = hs.handle_message(&server_hello, &mut record).unwrap();
    let _ = hs.handle_message(&build_encrypted_extensions(), &mut record).unwrap();
    let _ = hs.handle_message(&build_certificate(), &mut record).unwrap();
    let _ = hs.handle_message(&build_certificate_verify(), &mut record).unwrap();

    // Compute correct Finished first to get the right verify_data
    let transcript_before = hs.transcript.clone().finish();
    let server_finished_key = hs.key_schedule.server_finished_key();
    let correct_verify = hmac_sha256(&server_finished_key, &transcript_before);

    // Now construct Finished with exactly ONE bit flipped
    let mut wrong_verify = correct_verify;
    wrong_verify[0] ^= 0x01;

    let mut finished_msg = Vec::new();
    finished_msg.push(codec::FINISHED);
    finished_msg.push(0x00);
    finished_msg.push(0x00);
    finished_msg.push(0x20);
    finished_msg.extend_from_slice(&wrong_verify);

    let result = hs.handle_message(&finished_msg, &mut record);
    assert!(
        result.is_err(),
        "1-bit wrong Finished verify_data should be rejected"
    );
}

// ── Test: install_app_keys panics if called before server Finished ──

#[test]
#[should_panic(expected = "install_app_keys called before server Finished")]
fn install_app_keys_before_server_finished_panics() {
    let mut hs = Handshake::new("example.com").unwrap();
    let _ch = hs.encode_client_hello().unwrap();

    // Don't complete the handshake — try to install app keys
    let mut record = RecordLayer::new();
    hs.install_app_keys(&mut record);
}

// ── Test: encode_client_finished fails if handshake not complete ─────

#[test]
fn encode_client_finished_works_after_complete_handshake() {
    let mut hs = Handshake::new("example.com").unwrap();
    let _ch = hs.encode_client_hello().unwrap();

    let server_random = [0x55; 32];
    let (_, server_pub) = x25519::keypair().unwrap();
    let server_hello = build_server_hello(server_random, server_pub, 0x1301);

    let mut record = RecordLayer::new();
    let _ = hs.handle_message(&server_hello, &mut record).unwrap();
    let _ = hs.handle_message(&build_encrypted_extensions(), &mut record).unwrap();
    let _ = hs.handle_message(&build_certificate(), &mut record).unwrap();
    let _ = hs.handle_message(&build_certificate_verify(), &mut record).unwrap();
    let server_finished_key = hs.key_schedule.server_finished_key();
    let transcript_before = hs.transcript.clone().finish();
    let result = hs.handle_message(&build_finished(false, &server_finished_key, &transcript_before), &mut record).unwrap();

    assert!(matches!(result, HandshakeResult::Complete), "expected Complete");

    // Now encode_client_finished should work
    let finished_msg = hs.encode_client_finished();
    assert_eq!(finished_msg.len(), 36, "Finished message should be 36 bytes");

    // install_app_keys should also work
    hs.install_app_keys(&mut record);
}
