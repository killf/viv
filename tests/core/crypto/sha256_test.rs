// SHA-256 / HMAC-SHA256 / HKDF / getrandom tests

use viv::core::crypto::sha256::{hkdf_expand, hkdf_extract, hmac_sha256, Sha256};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

// ── getrandom ────────────────────────────────────────────────────────

#[test]
fn getrandom_fills_buffer() {
    let mut buf = [0u8; 32];
    viv::core::crypto::getrandom(&mut buf).unwrap();
    assert_ne!(buf, [0u8; 32]);
}

// ── SHA-256 (FIPS 180-4) ────────────────────────────────────────────

#[test]
fn sha256_empty() {
    // FIPS 180-4 4.1
    assert_eq!(hex(&Sha256::hash(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

#[test]
fn sha256_abc() {
    // FIPS 180-4 4.1
    assert_eq!(
        hex(&Sha256::hash(b"abc")),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn sha256_448bit() {
    // FIPS 180-4 4.1
    let input = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
    assert_eq!(
        hex(&Sha256::hash(input)),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

#[test]
fn sha256_incremental() {
    let mut hasher = Sha256::new();
    hasher.update(b"abc");
    hasher.update(b"dbcdecdefdefg");
    hasher.update(b"efghfghighijhijkijkljklmklmnlmnomnopnopq");
    let incremental = hasher.finish();
    let oneshot = Sha256::hash(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq");
    assert_eq!(hex(&incremental), hex(&oneshot));
}

#[test]
fn sha256_clone() {
    let mut h = Sha256::new();
    h.update(b"abc");
    let fork = h.clone();
    h.update(b"def");
    let full = h.finish();
    let partial = fork.finish();
    assert_eq!(hex(&partial), hex(&Sha256::hash(b"abc")));
    assert_eq!(hex(&full), hex(&Sha256::hash(b"abcdef")));
    assert_ne!(partial, full);
}

// ── HMAC-SHA256 (RFC 4231) ───────────────────────────────────────────

#[test]
fn hmac_sha256_rfc4231_tc2() {
    // RFC 4231 Test Case 2: key = "Jefe", data = "what do ya want for nothing?"
    let mac = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
    assert_eq!(
        hex(&mac),
        "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
    );
}

// ── HKDF (RFC 5869) ─────────────────────────────────────────────────

#[test]
fn hkdf_extract_rfc5869_tc1() {
    // RFC 5869 Test Case 1
    let ikm = [0x0bu8; 22];
    let salt: Vec<u8> = (0x00..=0x0cu8).collect();
    let prk = hkdf_extract(&salt, &ikm);
    assert_eq!(
        hex(&prk),
        "077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5"
    );
}

#[test]
fn hkdf_expand_rfc5869_tc1() {
    // RFC 5869 Test Case 1
    let prk = [
        0x07, 0x77, 0x09, 0x36, 0x2c, 0x2e, 0x32, 0xdf, 0x0d, 0xdc, 0x3f, 0x0d, 0xc4, 0x7b,
        0xba, 0x63, 0x90, 0xb6, 0xc7, 0x3b, 0xb5, 0x0f, 0x9c, 0x31, 0x22, 0xec, 0x84, 0x4a,
        0xd7, 0xc2, 0xb3, 0xe5,
    ];
    let info: Vec<u8> = (0xf0..=0xf9u8).collect();
    let mut okm = [0u8; 42];
    let _ = hkdf_expand(&prk, &info, &mut okm);
    assert_eq!(
        hex(&okm),
        "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
    );
}
