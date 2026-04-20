// SHA-1 (FIPS 180-4) and HMAC-SHA1 (RFC 2104) tests
//
// Test vectors from Python hashlib (SHA-1) and hmac module (HMAC-SHA1).

use viv::core::crypto::sha1::{hmac_sha1, Sha1};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

// ── SHA-1 (FIPS 180-4) ────────────────────────────────────────────

#[test]
fn test_sha1_empty() {
    assert_eq!(hex(&Sha1::hash(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
}

#[test]
fn test_sha1_abc() {
    assert_eq!(hex(&Sha1::hash(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
}

#[test]
fn test_sha1_64_zeros() {
    let input = [0u8; 64];
    assert_eq!(hex(&Sha1::hash(&input)), "c8d7d0ef0eedfa82d2ea1aa592845b9a6d4b02b7");
}

#[test]
fn test_sha1_abc_multiblock() {
    let input: Vec<u8> = "abc".repeat(16).into();
    assert_eq!(hex(&Sha1::hash(&input)), "b0b366cdb3ce23b1806c6d542e8b2edbb2876811");
}

#[test]
fn test_sha1_incremental_api() {
    let data = [0u8; 100];
    let direct = Sha1::hash(&data);
    let mut inc = Sha1::new();
    inc.update(&data[..50]);
    inc.update(&data[50..]);
    assert_eq!(hex(&inc.finish()), hex(&direct));
}

// ── HMAC-SHA1 ───────────────────────────────────────────────────

#[test]
fn test_hmac_sha1_empty() {
    assert_eq!(hex(&hmac_sha1(b"", b"")), "fbdb1d1b18aa6c08324b7d64b71fb76370690e1d");
}

#[test]
fn test_hmac_sha1_key_fox() {
    assert_eq!(
        hex(&hmac_sha1(b"key", b"The quick brown fox jumps over the lazy dog")),
        "de7c9b85b8b78aa6bc8a7a36f70a90701c9db4d9"
    );
}

#[test]
fn test_hmac_sha1_long_key_test() {
    assert_eq!(
        hex(&hmac_sha1(&[0xaau8; 20], b"Test Using Larger Than Block-Size Key - Hash Key First")),
        "9cb23930f1c54223ca1d0f9b4522b768cf008658"
    );
}

#[test]
fn test_hmac_sha1_long_key_sample4() {
    assert_eq!(
        hex(&hmac_sha1(&[0xaau8; 20], b"Sample #4")),
        "890854dcf525b9f6c9aefaead5a31fb0360c7ef1"
    );
}

#[test]
fn test_hmac_sha1_key_newline() {
    assert_eq!(
        hex(&hmac_sha1(b"key\n", b"Test With Truncation")),
        "0d361545d912513e14a2fad907a32e352f56d7a6"
    );
}

#[test]
fn test_hmac_sha1_longer_than_block_key() {
    // Key longer than 64 bytes — should be hashed first
    let key = [0xaau8; 100];
    let data = b"test data";
    let hash = hmac_sha1(&key, data);
    let hash2 = hmac_sha1(&key, data);
    assert_eq!(hash, hash2);
    assert_ne!(hash, [0u8; 20]);
}

#[test]
fn test_hmac_sha1_long_key_larger() {
    // 25-byte key (> 20 bytes of 0xaa)
    assert_eq!(
        hex(&hmac_sha1(&[0xaau8; 25], b"Sample Using Larger Than Block-Size Key - Hash Key First")),
        "3ee3de4f6645aac17587f378ff7ef35a3f72875b"
    );
}
