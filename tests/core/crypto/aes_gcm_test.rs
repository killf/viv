// AES-128-GCM tests — NIST SP 800-38A (AES ECB) + SP 800-38D (GCM)

use viv::core::crypto::aes_gcm::{Aes128, Aes128Gcm};

// ── AES-128 ECB (NIST SP 800-38A F.1.1) ────────────────────────────

#[test]
fn aes128_ecb_nist_f11() {
    let key = [
        0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf, 0x4f,
        0x3c,
    ];
    let pt = [
        0x6b, 0xc1, 0xbe, 0xe2, 0x2e, 0x40, 0x9f, 0x96, 0xe9, 0x3d, 0x7e, 0x11, 0x73, 0x93, 0x17,
        0x2a,
    ];
    let expected_ct = [
        0x3a, 0xd7, 0x7b, 0xb4, 0x0d, 0x7a, 0x36, 0x60, 0xa8, 0x9e, 0xca, 0xf3, 0x24, 0x66, 0xef,
        0x97,
    ];
    let aes = Aes128::new(&key);
    let ct = aes.encrypt_block(&pt);
    assert_eq!(ct, expected_ct);
}

#[test]
fn aes128_ecb_nist_f11_block2() {
    let key = [
        0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf, 0x4f,
        0x3c,
    ];
    let pt = [
        0xae, 0x2d, 0x8a, 0x57, 0x1e, 0x03, 0xac, 0x9c, 0x9e, 0xb7, 0x6f, 0xac, 0x45, 0xaf, 0x8e,
        0x51,
    ];
    let expected_ct = [
        0xf5, 0xd3, 0xd5, 0x85, 0x03, 0xb9, 0x69, 0x9d, 0xe7, 0x85, 0x89, 0x5a, 0x96, 0xfd, 0xba,
        0xaf,
    ];
    let aes = Aes128::new(&key);
    let ct = aes.encrypt_block(&pt);
    assert_eq!(ct, expected_ct);
}

// ── GCM Test Case 1: empty plaintext, empty AAD ────────────────────

#[test]
fn gcm_tc1_empty_pt_empty_aad() {
    let key = [0u8; 16];
    let nonce = [0u8; 12];
    let expected_tag = [
        0x58, 0xe2, 0xfc, 0xce, 0xfa, 0x7e, 0x30, 0x61, 0x36, 0x7f, 0x1d, 0x57, 0xa4, 0xe7, 0x45,
        0x5a,
    ];

    // Encrypt with empty plaintext → output is just the 16-byte tag
    let mut out = [0u8; 16];
    let gcm = Aes128Gcm::new(&key);
    let _ = gcm.encrypt(&nonce, &[], &[], &mut out);
    assert_eq!(&out[..16], &expected_tag);
}

// ── GCM Test Case 2: 16-byte plaintext, empty AAD ──────────────────

#[test]
fn gcm_tc2_16byte_pt_empty_aad() {
    let key = [0u8; 16];
    let nonce = [0u8; 12];
    let pt = [0u8; 16];
    let expected_ct = [
        0x03, 0x88, 0xda, 0xce, 0x60, 0xb6, 0xa3, 0x92, 0xf3, 0x28, 0xc2, 0xb9, 0x71, 0xb2, 0xfe,
        0x78,
    ];
    let expected_tag = [
        0xab, 0x6e, 0x47, 0xd4, 0x2c, 0xec, 0x13, 0xbd, 0xf5, 0x3a, 0x67, 0xb2, 0x12, 0x57, 0xbd,
        0xdf,
    ];

    let gcm = Aes128Gcm::new(&key);
    let mut out = [0u8; 32]; // 16 ct + 16 tag
    let _ = gcm.encrypt(&nonce, &[], &pt, &mut out);
    assert_eq!(&out[..16], &expected_ct);
    assert_eq!(&out[16..32], &expected_tag);
}

// ── GCM decrypt roundtrip ──────────────────────────────────────────

#[test]
fn gcm_decrypt_roundtrip() {
    let key = [0u8; 16];
    let nonce = [0u8; 12];
    let pt = [0u8; 16];

    let gcm = Aes128Gcm::new(&key);

    // Encrypt
    let mut ct_and_tag = [0u8; 32];
    let _ = gcm.encrypt(&nonce, &[], &pt, &mut ct_and_tag);

    // Decrypt
    let mut decrypted = [0u8; 16];
    let n = gcm
        .decrypt(&nonce, &[], &ct_and_tag, &mut decrypted)
        .unwrap();
    assert_eq!(n, 16);
    assert_eq!(&decrypted[..n], &pt);
}

// ── GCM tampered tag → error ────────────────────────────────────────

#[test]
fn gcm_tampered_tag_rejected() {
    let key = [0u8; 16];
    let nonce = [0u8; 12];
    let pt = [0u8; 16];

    let gcm = Aes128Gcm::new(&key);

    let mut ct_and_tag = [0u8; 32];
    let _ = gcm.encrypt(&nonce, &[], &pt, &mut ct_and_tag);

    // Flip one bit in the tag
    ct_and_tag[31] ^= 0x01;

    let mut decrypted = [0u8; 16];
    let result = gcm.decrypt(&nonce, &[], &ct_and_tag, &mut decrypted);
    assert!(result.is_err());
}

// ── GCM with AAD ────────────────────────────────────────────────────

#[test]
fn gcm_encrypt_decrypt_with_aad() {
    let key = [0x42u8; 16];
    let nonce = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c,
    ];
    let aad = b"additional authenticated data";
    let pt = b"hello, world!!!!"; // 16 bytes

    let gcm = Aes128Gcm::new(&key);

    let mut ct_and_tag = [0u8; 32];
    let _ = gcm.encrypt(&nonce, aad, pt, &mut ct_and_tag);

    // Decrypt should succeed and produce original plaintext
    let mut decrypted = [0u8; 16];
    let n = gcm
        .decrypt(&nonce, aad, &ct_and_tag, &mut decrypted)
        .unwrap();
    assert_eq!(n, 16);
    assert_eq!(&decrypted[..n], pt);

    // Wrong AAD should fail
    let mut decrypted2 = [0u8; 16];
    let result = gcm.decrypt(&nonce, b"wrong aad", &ct_and_tag, &mut decrypted2);
    assert!(result.is_err());
}

// ── GCM TC1 decrypt (empty ciphertext, tag only) ───────────────────

#[test]
fn gcm_tc1_decrypt_empty() {
    let key = [0u8; 16];
    let nonce = [0u8; 12];
    let tag = [
        0x58, 0xe2, 0xfc, 0xce, 0xfa, 0x7e, 0x30, 0x61, 0x36, 0x7f, 0x1d, 0x57, 0xa4, 0xe7, 0x45,
        0x5a,
    ];

    let gcm = Aes128Gcm::new(&key);
    let mut decrypted = [0u8; 0];
    let n = gcm.decrypt(&nonce, &[], &tag, &mut decrypted).unwrap();
    assert_eq!(n, 0);
}

// ── GCM non-block-aligned plaintext ─────────────────────────────────

#[test]
fn gcm_non_aligned_plaintext() {
    let key = [0xabu8; 16];
    let nonce = [0xcdu8; 12];
    let pt = b"seven!!"; // 7 bytes, not a multiple of 16

    let gcm = Aes128Gcm::new(&key);

    let mut ct_and_tag = [0u8; 7 + 16]; // 7 ct + 16 tag
    let _ = gcm.encrypt(&nonce, &[], pt, &mut ct_and_tag);

    let mut decrypted = [0u8; 7];
    let n = gcm
        .decrypt(&nonce, &[], &ct_and_tag, &mut decrypted)
        .unwrap();
    assert_eq!(n, 7);
    assert_eq!(&decrypted[..n], pt);
}
