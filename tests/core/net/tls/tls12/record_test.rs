use viv::core::net::tls::tls12::record::Tls12RecordLayer;
use viv::core::net::tls::tls12::key_schedule::KeyBlock;

fn test_key_block() -> KeyBlock {
    KeyBlock {
        client_write_key: [0x01u8; 16],
        server_write_key: [0x02u8; 16],
        client_write_iv: [0x03u8; 4],
        server_write_iv: [0x04u8; 4],
    }
}

#[test]
fn encrypt_then_decrypt_roundtrip() {
    let kb = test_key_block();
    let mut client = Tls12RecordLayer::new();
    let mut server = Tls12RecordLayer::new();
    client.install_client_keys(&kb);
    server.install_server_keys(&kb);

    let plaintext = b"hello tls12";
    let mut record = Vec::new();
    client.write_encrypted(0x17, plaintext, &mut record).unwrap();

    let (ct, payload, consumed) = server.read_record(&record).unwrap();
    assert_eq!(consumed, record.len());
    assert_eq!(ct, 0x17);
    assert_eq!(payload, plaintext);
}

#[test]
fn encrypted_record_contains_explicit_iv() {
    let kb = test_key_block();
    let mut client = Tls12RecordLayer::new();
    client.install_client_keys(&kb);

    let plaintext = b"test";
    let mut record = Vec::new();
    client.write_encrypted(0x17, plaintext, &mut record).unwrap();

    // TLS 1.2 record: header(5) + explicit_iv(8) + ciphertext(n) + tag(16)
    assert!(record.len() >= 5 + 8 + plaintext.len() + 16);
}

#[test]
fn sequence_number_produces_distinct_ciphertexts() {
    let kb = test_key_block();
    let mut client = Tls12RecordLayer::new();
    client.install_client_keys(&kb);

    let plaintext = b"same message";
    let mut r1 = Vec::new();
    let mut r2 = Vec::new();
    client.write_encrypted(0x17, plaintext, &mut r1).unwrap();
    client.write_encrypted(0x17, plaintext, &mut r2).unwrap();
    assert_ne!(r1, r2);
}
