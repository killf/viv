// Record layer tests — verify TLS record framing and AEAD roundtrip.

use viv::core::net::tls::codec;
use viv::core::net::tls::record::RecordLayer;

// ── Plaintext record format ────────────────────────────────────────

#[test]
fn write_plaintext_format() {
    let record = RecordLayer::new();
    let payload = b"hello";
    let mut out = Vec::new();

    record.write_plaintext(codec::HANDSHAKE, payload, &mut out);

    // content_type = 0x16 (Handshake)
    assert_eq!(out[0], 0x16);

    // version = 0x0301 (TLS 1.0 compat)
    assert_eq!(out[1], 0x03);
    assert_eq!(out[2], 0x01);

    // length = 5
    assert_eq!(out[3], 0x00);
    assert_eq!(out[4], 0x05);

    // payload
    assert_eq!(&out[5..], b"hello");
}

// ── Encrypt-then-decrypt roundtrip ─────────────────────────────────

#[test]
fn encrypt_decrypt_roundtrip() {
    let key = [0x01u8; 16];
    let iv = [0x02u8; 12];

    let mut writer = RecordLayer::new();
    writer.install_encrypter(key, iv);

    let mut reader = RecordLayer::new();
    reader.install_decrypter(key, iv);

    let payload = b"Hello, TLS 1.3!";
    let mut encrypted = Vec::new();
    writer.write_encrypted(codec::APPLICATION_DATA, payload, &mut encrypted);

    // The encrypted record should have outer type APPLICATION_DATA (0x17)
    assert_eq!(encrypted[0], 0x17);

    // version should be 0x0303
    assert_eq!(encrypted[1], 0x03);
    assert_eq!(encrypted[2], 0x03);

    // Decrypt
    let (ct, decrypted, consumed) = reader
        .read_record(&encrypted)
        .expect("decrypt should succeed");

    assert_eq!(ct, codec::APPLICATION_DATA);
    assert_eq!(decrypted, payload);
    assert_eq!(consumed, encrypted.len());
}

#[test]
fn encrypt_decrypt_multiple_records() {
    let key = [0x0A; 16];
    let iv = [0x0B; 12];

    let mut writer = RecordLayer::new();
    writer.install_encrypter(key, iv);

    let mut reader = RecordLayer::new();
    reader.install_decrypter(key, iv);

    // Encrypt two records to verify nonce increments correctly
    let msg1 = b"first message";
    let msg2 = b"second message";

    let mut enc1 = Vec::new();
    writer.write_encrypted(codec::APPLICATION_DATA, msg1, &mut enc1);

    let mut enc2 = Vec::new();
    writer.write_encrypted(codec::APPLICATION_DATA, msg2, &mut enc2);

    // Decrypt first
    let (ct1, dec1, consumed1) = reader.read_record(&enc1).expect("decrypt first");
    assert_eq!(ct1, codec::APPLICATION_DATA);
    assert_eq!(dec1, msg1);
    assert_eq!(consumed1, enc1.len());

    // Decrypt second (nonce should have advanced)
    let (ct2, dec2, consumed2) = reader.read_record(&enc2).expect("decrypt second");
    assert_eq!(ct2, codec::APPLICATION_DATA);
    assert_eq!(dec2, msg2);
    assert_eq!(consumed2, enc2.len());
}

#[test]
fn nonce_changes_between_records() {
    let key = [0xFF; 16];
    let iv = [0x00; 12];

    let mut writer = RecordLayer::new();
    writer.install_encrypter(key, iv);

    let payload = b"test";

    // Encrypt two identical payloads — ciphertext should differ due to nonce
    let mut enc1 = Vec::new();
    writer.write_encrypted(codec::APPLICATION_DATA, payload, &mut enc1);

    let mut enc2 = Vec::new();
    writer.write_encrypted(codec::APPLICATION_DATA, payload, &mut enc2);

    // The encrypted payloads (after the 5-byte header) should differ
    assert_ne!(
        &enc1[5..],
        &enc2[5..],
        "nonce should produce different ciphertexts"
    );
}

#[test]
fn read_plaintext_record() {
    let mut record = RecordLayer::new();

    // Build a plaintext record manually: type(0x16) + version(0x0301) + len + payload
    let payload = b"data";
    let mut data = Vec::new();
    data.push(codec::HANDSHAKE);
    data.push(0x03);
    data.push(0x01);
    data.push(0x00);
    data.push(payload.len() as u8);
    data.extend_from_slice(payload);

    let (ct, body, consumed) = record.read_record(&data).expect("read should succeed");
    assert_eq!(ct, codec::HANDSHAKE);
    assert_eq!(body, payload);
    assert_eq!(consumed, data.len());
}

#[test]
fn read_record_too_short_returns_error() {
    let mut record = RecordLayer::new();
    let data = [0x16, 0x03, 0x01]; // Only 3 bytes, need at least 5
    assert!(record.read_record(&data).is_err());
}

#[test]
fn encrypted_inner_content_type_preserved() {
    let key = [0x42; 16];
    let iv = [0x43; 12];

    let mut writer = RecordLayer::new();
    writer.install_encrypter(key, iv);

    let mut reader = RecordLayer::new();
    reader.install_decrypter(key, iv);

    // Encrypt a HANDSHAKE message (inner type should be preserved)
    let payload = b"handshake data";
    let mut encrypted = Vec::new();
    writer.write_encrypted(codec::HANDSHAKE, payload, &mut encrypted);

    // Outer type should be APPLICATION_DATA
    assert_eq!(encrypted[0], codec::APPLICATION_DATA);

    // After decryption, inner type should be HANDSHAKE
    let (ct, decrypted, _) = reader.read_record(&encrypted).expect("decrypt");
    assert_eq!(ct, codec::HANDSHAKE);
    assert_eq!(decrypted, payload);
}
