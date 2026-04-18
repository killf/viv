# TLS 1.3 Self-Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace OpenSSL FFI with a pure Rust TLS 1.3 client — zero external dependencies.

**Architecture:** Bottom-up build: crypto primitives → key schedule → record layer → codec → handshake state machine → TlsStream integration. Each layer is independently testable with RFC standard test vectors. The public API (`TlsStream::connect`, `AsyncTlsStream::connect`) stays identical — callers need zero changes.

**Tech Stack:** Pure Rust (edition 2024), no crates. Crypto: SHA-256 (FIPS 180-4), HMAC (RFC 2104), HKDF (RFC 5869), AES-128-GCM (FIPS 197 + SP 800-38D), X25519 (RFC 7748). Protocol: TLS 1.3 (RFC 8446), cipher suite `TLS_AES_128_GCM_SHA256`.

**Spec:** `docs/superpowers/specs/2026-04-18-tls13-self-impl-design.md`

**Reference code:** `/data/dlab/rustls-ref/` (rustls clone)

---

## File Map

### New files (create)

| File | Responsibility |
|------|---------------|
| `src/core/net/tls/mod.rs` | `TlsStream`, `AsyncTlsStream` — public API |
| `src/core/net/tls/crypto/mod.rs` | `getrandom()` syscall |
| `src/core/net/tls/crypto/sha256.rs` | SHA-256 + HMAC-SHA256 + HKDF |
| `src/core/net/tls/crypto/aes_gcm.rs` | AES-128-GCM (AEAD) |
| `src/core/net/tls/crypto/x25519.rs` | X25519 ECDHE key exchange |
| `src/core/net/tls/key_schedule.rs` | TLS 1.3 key derivation chain |
| `src/core/net/tls/codec.rs` | TLS message encode/decode |
| `src/core/net/tls/record.rs` | TLS record layer (framing + AEAD) |
| `src/core/net/tls/handshake.rs` | TLS 1.3 client handshake state machine |
| `src/core/net/tls/x509.rs` | Certificate stub (skip verification) |
| `tests/core/net/tls/mod.rs` | Test module root |
| `tests/core/net/tls/crypto/mod.rs` | Crypto test module root |
| `tests/core/net/tls/crypto/sha256_test.rs` | SHA-256/HMAC/HKDF tests |
| `tests/core/net/tls/crypto/aes_gcm_test.rs` | AES-128-GCM tests |
| `tests/core/net/tls/crypto/x25519_test.rs` | X25519 tests |
| `tests/core/net/tls/key_schedule_test.rs` | Key schedule tests (RFC 8448) |
| `tests/core/net/tls/codec_test.rs` | Codec encode/decode tests |
| `tests/core/net/tls/record_test.rs` | Record layer tests |
| `tests/core/net/tls/handshake_test.rs` | Integration test (full_test) |

### Modify

| File | Change |
|------|--------|
| `src/core/net/mod.rs` | Change `pub mod tls;` to directory module |
| `tests/core/net/mod.rs` | Replace `mod tls_test;` with `mod tls;` |

### Delete (after migration)

| File |
|------|
| `src/core/net/tls.rs` |
| `src/core/net/async_tls.rs` |
| `tests/core/net/tls_test.rs` |

---

## Task 1: Project Scaffolding

**Files:**
- Create: `src/core/net/tls/mod.rs`, `src/core/net/tls/crypto/mod.rs`, `src/core/net/tls/x509.rs`
- Create: `tests/core/net/tls/mod.rs`, `tests/core/net/tls/crypto/mod.rs`
- Modify: `src/core/net/mod.rs`, `tests/core/net/mod.rs`

- [ ] **Step 1: Create source directory structure with stub modules**

```rust
// src/core/net/tls/crypto/mod.rs
use std::arch::asm;

pub mod sha256;
pub mod aes_gcm;
pub mod x25519;

/// Fill `buf` with cryptographically secure random bytes.
/// Uses Linux getrandom() syscall directly (syscall 318, x86_64).
pub fn getrandom(buf: &mut [u8]) -> crate::Result<()> {
    let ret: i64;
    unsafe {
        asm!(
            "syscall",
            in("rax") 318u64,          // SYS_getrandom
            in("rdi") buf.as_mut_ptr(),
            in("rsi") buf.len(),
            in("rdx") 0u64,            // flags = 0
            lateout("rax") ret,
            lateout("rcx") _,
            lateout("r11") _,
        );
    }
    if ret < 0 {
        Err(crate::Error::Tls("getrandom syscall failed".into()))
    } else {
        Ok(())
    }
}
```

```rust
// src/core/net/tls/crypto/sha256.rs
// (empty — Task 2)
```

```rust
// src/core/net/tls/crypto/aes_gcm.rs
// (empty — Task 4)
```

```rust
// src/core/net/tls/crypto/x25519.rs
// (empty — Task 6)
```

```rust
// src/core/net/tls/key_schedule.rs
// (empty — Task 7)
```

```rust
// src/core/net/tls/codec.rs
// (empty — Task 8)
```

```rust
// src/core/net/tls/record.rs
// (empty — Task 9)
```

```rust
// src/core/net/tls/handshake.rs
// (empty — Task 10)
```

```rust
// src/core/net/tls/x509.rs
// Certificate verification stub — skip for now.
```

```rust
// src/core/net/tls/mod.rs
pub mod crypto;
pub mod key_schedule;
pub mod codec;
pub mod record;
pub mod handshake;
pub mod x509;

// Re-export old TlsStream/AsyncTlsStream for compatibility during migration.
// These will be replaced in Task 11.
```

- [ ] **Step 2: Create test directory structure**

```rust
// tests/core/net/tls/crypto/mod.rs
mod sha256_test;
mod aes_gcm_test;
mod x25519_test;
```

```rust
// tests/core/net/tls/mod.rs
mod crypto;
mod key_schedule_test;
mod codec_test;
mod record_test;
```

Create empty test files:
- `tests/core/net/tls/crypto/sha256_test.rs`
- `tests/core/net/tls/crypto/aes_gcm_test.rs`
- `tests/core/net/tls/crypto/x25519_test.rs`
- `tests/core/net/tls/key_schedule_test.rs`
- `tests/core/net/tls/codec_test.rs`
- `tests/core/net/tls/record_test.rs`

- [ ] **Step 3: Wire up module system**

Modify `src/core/net/mod.rs` — keep old modules temporarily during migration:

```rust
pub mod async_tcp;
pub mod async_tls;  // keep old for now
pub mod http;
pub mod sse;
pub mod tcp;
pub mod tls;        // now points to tls/ directory
pub mod ws;
```

Modify `tests/core/net/mod.rs`:

```rust
pub mod http_test;
pub mod sse_test;
mod tls;   // was: mod tls_test;
mod ws_test;
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build 2>&1 | head -20`
Expected: Compiles with no errors (old TLS code still in `async_tls.rs`, new `tls/` is stubs).

Note: The old `src/core/net/tls.rs` file conflicts with `src/core/net/tls/` directory. You must **rename** the old file first (e.g., `tls_old.rs`) or delete it and temporarily re-export from the new `tls/mod.rs`. The recommended approach: move the old `TlsStream` into `tls/mod.rs` temporarily as a re-export, delete `tls.rs`.

- [ ] **Step 5: Verify getrandom works**

Add to `tests/core/net/tls/crypto/sha256_test.rs` (temporary, just to test getrandom):

```rust
#[test]
fn getrandom_fills_buffer() {
    let mut buf = [0u8; 32];
    viv::core::net::tls::crypto::getrandom(&mut buf).unwrap();
    // Extremely unlikely all 32 bytes remain zero
    assert_ne!(buf, [0u8; 32]);
}
```

Run: `cargo test getrandom_fills_buffer`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/core/net/tls/ tests/core/net/tls/
git commit -m "scaffold: TLS 1.3 self-impl directory structure + getrandom"
```

---

## Task 2: SHA-256

**Files:**
- Create: `src/core/net/tls/crypto/sha256.rs`
- Test: `tests/core/net/tls/crypto/sha256_test.rs`

**Reference:** FIPS 180-4, RFC 4231 (HMAC), RFC 5869 (HKDF)

- [ ] **Step 1: Write SHA-256 failing tests**

```rust
// tests/core/net/tls/crypto/sha256_test.rs
use viv::core::net::tls::crypto::sha256::Sha256;

/// NIST FIPS 180-4: "abc"
#[test]
fn sha256_abc() {
    let digest = Sha256::hash(b"abc");
    let expected: [u8; 32] = [
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea,
        0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22, 0x23,
        0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c,
        0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00, 0x15, 0xad,
    ];
    assert_eq!(digest, expected);
}

/// NIST FIPS 180-4: empty string
#[test]
fn sha256_empty() {
    let digest = Sha256::hash(b"");
    let expected: [u8; 32] = [
        0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14,
        0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9, 0x24,
        0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c,
        0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52, 0xb8, 0x55,
    ];
    assert_eq!(digest, expected);
}

/// NIST FIPS 180-4: 448-bit message (56 bytes — exactly fills one block after padding)
#[test]
fn sha256_448bit() {
    let digest = Sha256::hash(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq");
    let expected: [u8; 32] = [
        0x24, 0x8d, 0x6a, 0x61, 0xd2, 0x06, 0x38, 0xb8,
        0xe5, 0xc0, 0x26, 0x93, 0x0c, 0x3e, 0x60, 0x39,
        0xa3, 0x3c, 0xe4, 0x59, 0x64, 0xff, 0x21, 0x67,
        0xf6, 0xec, 0xed, 0xd4, 0x19, 0xdb, 0x06, 0xc1,
    ];
    assert_eq!(digest, expected);
}

/// Incremental hashing matches one-shot
#[test]
fn sha256_incremental() {
    let mut h = Sha256::new();
    h.update(b"abc");
    h.update(b"dbcdecdefdefg");
    h.update(b"efghfghighijhijkijkljklmklmnlmnomnopnopq");
    let digest = h.finish();
    let one_shot = Sha256::hash(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq");
    assert_eq!(digest, one_shot);
}

/// Clone preserves state (needed for transcript forking)
#[test]
fn sha256_clone() {
    let mut h = Sha256::new();
    h.update(b"abc");
    let h2 = h.clone();
    h.update(b"def");
    let d1 = h.finish();
    let d2 = h2.finish();
    assert_eq!(d1, Sha256::hash(b"abcdef"));
    assert_eq!(d2, Sha256::hash(b"abc"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test sha256_ 2>&1 | tail -5`
Expected: FAIL — `Sha256` not found

- [ ] **Step 3: Implement SHA-256**

Write `src/core/net/tls/crypto/sha256.rs`:

```rust
/// SHA-256 constants: first 32 bits of fractional parts of cube roots of first 64 primes
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
    0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
    0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
    0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
    0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

#[derive(Clone)]
pub struct Sha256 {
    state: [u32; 8],
    buf: [u8; 64],
    buf_len: usize,
    total_len: u64,
}

impl Sha256 {
    pub fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
            ],
            buf: [0; 64],
            buf_len: 0,
            total_len: 0,
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        self.total_len += data.len() as u64;
        let mut offset = 0;

        // If we have buffered data, try to fill it
        if self.buf_len > 0 {
            let need = 64 - self.buf_len;
            if data.len() < need {
                self.buf[self.buf_len..self.buf_len + data.len()]
                    .copy_from_slice(data);
                self.buf_len += data.len();
                return;
            }
            self.buf[self.buf_len..64].copy_from_slice(&data[..need]);
            let block = self.buf;
            compress(&mut self.state, &block);
            self.buf_len = 0;
            offset = need;
        }

        // Process full blocks
        while offset + 64 <= data.len() {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[offset..offset + 64]);
            compress(&mut self.state, &block);
            offset += 64;
        }

        // Buffer remainder
        let remaining = data.len() - offset;
        if remaining > 0 {
            self.buf[..remaining].copy_from_slice(&data[offset..]);
            self.buf_len = remaining;
        }
    }

    pub fn finish(mut self) -> [u8; 32] {
        let bit_len = self.total_len * 8;

        // Padding: 1 bit, then zeros, then 64-bit length
        let mut pad = [0u8; 72]; // max padding: 64 + 8
        pad[0] = 0x80;
        let pad_len = if self.buf_len < 56 {
            56 - self.buf_len
        } else {
            120 - self.buf_len
        };
        self.update(&pad[..pad_len]);
        self.update(&bit_len.to_be_bytes());

        // Output
        let mut out = [0u8; 32];
        for (i, word) in self.state.iter().enumerate() {
            out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }

    /// One-shot hash
    pub fn hash(data: &[u8]) -> [u8; 32] {
        let mut h = Self::new();
        h.update(data);
        h.finish()
    }
}

fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([
            block[i * 4],
            block[i * 4 + 1],
            block[i * 4 + 2],
            block[i * 4 + 3],
        ]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }

    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;

    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let t1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}
```

- [ ] **Step 4: Run SHA-256 tests**

Run: `cargo test sha256_ -- --nocapture 2>&1 | tail -10`
Expected: All 5 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/core/net/tls/crypto/sha256.rs tests/core/net/tls/crypto/sha256_test.rs
git commit -m "feat(tls): implement SHA-256 with NIST test vectors"
```

---

## Task 3: HMAC-SHA256 + HKDF

**Files:**
- Modify: `src/core/net/tls/crypto/sha256.rs`
- Test: `tests/core/net/tls/crypto/sha256_test.rs`

- [ ] **Step 1: Write HMAC and HKDF failing tests**

```rust
// append to tests/core/net/tls/crypto/sha256_test.rs
use viv::core::net::tls::crypto::sha256::{hmac_sha256, hkdf_extract, hkdf_expand};

/// RFC 4231 Test Case 2: HMAC-SHA256
#[test]
fn hmac_sha256_rfc4231_tc2() {
    let key = b"Jefe";
    let data = b"what do ya want for nothing?";
    let expected: [u8; 32] = [
        0x5b, 0xdc, 0xc1, 0x46, 0xbf, 0x60, 0x75, 0x4e,
        0x6a, 0x04, 0x24, 0x26, 0x08, 0x95, 0x75, 0xc7,
        0x5a, 0x00, 0x3f, 0x08, 0x9d, 0x27, 0x39, 0x83,
        0x9d, 0xec, 0x58, 0xb9, 0x64, 0xec, 0x38, 0x43,
    ];
    assert_eq!(hmac_sha256(key, data), expected);
}

/// RFC 5869 Test Case 1: HKDF-Extract
#[test]
fn hkdf_extract_rfc5869_tc1() {
    let ikm: [u8; 22] = [0x0b; 22];
    let salt: [u8; 13] = [0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06,
                          0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c];
    let expected: [u8; 32] = [
        0x07, 0x77, 0x09, 0x36, 0x2c, 0x2e, 0x32, 0xdf,
        0x0d, 0xdc, 0x3f, 0x0d, 0xc4, 0x7b, 0xba, 0x63,
        0x90, 0xb6, 0xc7, 0x3b, 0xb5, 0x0f, 0x9c, 0x31,
        0x22, 0xec, 0x84, 0x4a, 0xd7, 0xc2, 0xb3, 0xe5,
    ];
    assert_eq!(hkdf_extract(&salt, &ikm), expected);
}

/// RFC 5869 Test Case 1: HKDF-Expand (L=42)
#[test]
fn hkdf_expand_rfc5869_tc1() {
    let prk: [u8; 32] = [
        0x07, 0x77, 0x09, 0x36, 0x2c, 0x2e, 0x32, 0xdf,
        0x0d, 0xdc, 0x3f, 0x0d, 0xc4, 0x7b, 0xba, 0x63,
        0x90, 0xb6, 0xc7, 0x3b, 0xb5, 0x0f, 0x9c, 0x31,
        0x22, 0xec, 0x84, 0x4a, 0xd7, 0xc2, 0xb3, 0xe5,
    ];
    let info: [u8; 10] = [0xf0, 0xf1, 0xf2, 0xf3, 0xf4,
                          0xf5, 0xf6, 0xf7, 0xf8, 0xf9];
    let expected: [u8; 42] = [
        0x3c, 0xb2, 0x5f, 0x25, 0xfa, 0xac, 0xd5, 0x7a,
        0x90, 0x43, 0x4f, 0x64, 0xd0, 0x36, 0x2f, 0x2a,
        0x2d, 0x2d, 0x0a, 0x90, 0xcf, 0x1a, 0x5a, 0x4c,
        0x5d, 0xb0, 0x2d, 0x56, 0xec, 0xc4, 0xc5, 0xbf,
        0x34, 0x00, 0x72, 0x08, 0xd5, 0xb8, 0x87, 0x18,
        0x58, 0x65,
    ];
    let mut out = [0u8; 42];
    hkdf_expand(&prk, &info, &mut out);
    assert_eq!(out, expected);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test hmac_sha256 hkdf_ 2>&1 | tail -5`
Expected: FAIL — functions not found

- [ ] **Step 3: Implement HMAC-SHA256 and HKDF**

Append to `src/core/net/tls/crypto/sha256.rs`:

```rust
/// RFC 2104: HMAC-SHA256
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    // If key > 64 bytes, hash it first
    let key_block = if key.len() > 64 {
        let h = Sha256::hash(key);
        let mut kb = [0u8; 64];
        kb[..32].copy_from_slice(&h);
        kb
    } else {
        let mut kb = [0u8; 64];
        kb[..key.len()].copy_from_slice(key);
        kb
    };

    // ipad = key XOR 0x36, opad = key XOR 0x5c
    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= key_block[i];
        opad[i] ^= key_block[i];
    }

    // inner = SHA256(ipad || data)
    let mut inner = Sha256::new();
    inner.update(&ipad);
    inner.update(data);
    let inner_hash = inner.finish();

    // outer = SHA256(opad || inner_hash)
    let mut outer = Sha256::new();
    outer.update(&opad);
    outer.update(&inner_hash);
    outer.finish()
}

/// RFC 5869: HKDF-Extract(salt, ikm) = HMAC-SHA256(salt, ikm)
pub fn hkdf_extract(salt: &[u8], ikm: &[u8]) -> [u8; 32] {
    let salt = if salt.is_empty() { &[0u8; 32] as &[u8] } else { salt };
    hmac_sha256(salt, ikm)
}

/// RFC 5869: HKDF-Expand(prk, info, L)
pub fn hkdf_expand(prk: &[u8], info: &[u8], out: &mut [u8]) {
    let n = (out.len() + 31) / 32; // ceil(L / HashLen)
    assert!(n <= 255, "HKDF-Expand: output too long");

    let mut t = Vec::new(); // T(i-1)
    let mut offset = 0;

    for i in 1..=n {
        // T(i) = HMAC(PRK, T(i-1) || info || i)
        let mut msg = Vec::with_capacity(t.len() + info.len() + 1);
        msg.extend_from_slice(&t);
        msg.extend_from_slice(info);
        msg.push(i as u8);
        let ti = hmac_sha256(prk, &msg);

        let copy_len = (out.len() - offset).min(32);
        out[offset..offset + copy_len].copy_from_slice(&ti[..copy_len]);
        offset += copy_len;

        t = ti.to_vec();
    }
}
```

- [ ] **Step 4: Run all SHA-256/HMAC/HKDF tests**

Run: `cargo test sha256_ hmac_ hkdf_ 2>&1 | tail -10`
Expected: All 8 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/core/net/tls/crypto/sha256.rs tests/core/net/tls/crypto/sha256_test.rs
git commit -m "feat(tls): implement HMAC-SHA256 + HKDF with RFC test vectors"
```

---

## Task 4: AES-128 Core

**Files:**
- Create: `src/core/net/tls/crypto/aes_gcm.rs`
- Test: `tests/core/net/tls/crypto/aes_gcm_test.rs`

- [ ] **Step 1: Write AES-128 single-block encrypt test**

```rust
// tests/core/net/tls/crypto/aes_gcm_test.rs
use viv::core::net::tls::crypto::aes_gcm::Aes128;

/// NIST SP 800-38A, Appendix F.1.1 (ECB-AES128, Block #1)
#[test]
fn aes128_single_block() {
    let key: [u8; 16] = [
        0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6,
        0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf, 0x4f, 0x3c,
    ];
    let plaintext: [u8; 16] = [
        0x6b, 0xc1, 0xbe, 0xe2, 0x2e, 0x40, 0x9f, 0x96,
        0xe9, 0x3d, 0x7e, 0x11, 0x73, 0x93, 0x17, 0x2a,
    ];
    let expected: [u8; 16] = [
        0x3a, 0xd7, 0x7b, 0xb4, 0x0d, 0x7a, 0x36, 0x60,
        0xa8, 0x9e, 0xca, 0xf3, 0x24, 0x66, 0xef, 0x97,
    ];
    let aes = Aes128::new(&key);
    let ciphertext = aes.encrypt_block(&plaintext);
    assert_eq!(ciphertext, expected);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test aes128_single_block 2>&1 | tail -5`
Expected: FAIL

- [ ] **Step 3: Implement AES-128 core**

Write `src/core/net/tls/crypto/aes_gcm.rs` — AES-128 portion:

```rust
const SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

const RCON: [u8; 11] = [
    0x00, 0x01, 0x02, 0x04, 0x08, 0x10,
    0x20, 0x40, 0x80, 0x1b, 0x36,
];

/// AES-128: 10 rounds, 16-byte key
pub struct Aes128 {
    round_keys: [[u8; 16]; 11],
}

impl Aes128 {
    pub fn new(key: &[u8; 16]) -> Self {
        let mut rk = [[0u8; 16]; 11];
        rk[0] = *key;

        for i in 1..11 {
            let prev = rk[i - 1];
            // RotWord + SubWord + RCON on last 4 bytes of previous round key
            let mut temp = [
                SBOX[prev[13] as usize] ^ RCON[i],
                SBOX[prev[14] as usize],
                SBOX[prev[15] as usize],
                SBOX[prev[12] as usize],
            ];
            for j in 0..4 {
                rk[i][j] = prev[j] ^ temp[j];
            }
            for j in 4..16 {
                rk[i][j] = prev[j] ^ rk[i][j - 4];
            }
        }

        Self { round_keys: rk }
    }

    /// Encrypt a single 16-byte block (ECB mode — used internally by CTR/GCM)
    pub fn encrypt_block(&self, block: &[u8; 16]) -> [u8; 16] {
        let mut state = *block;

        // Initial AddRoundKey
        xor_block(&mut state, &self.round_keys[0]);

        // Rounds 1..9: SubBytes + ShiftRows + MixColumns + AddRoundKey
        for round in 1..10 {
            sub_bytes(&mut state);
            shift_rows(&mut state);
            mix_columns(&mut state);
            xor_block(&mut state, &self.round_keys[round]);
        }

        // Round 10: SubBytes + ShiftRows + AddRoundKey (no MixColumns)
        sub_bytes(&mut state);
        shift_rows(&mut state);
        xor_block(&mut state, &self.round_keys[10]);

        state
    }
}

fn xor_block(a: &mut [u8; 16], b: &[u8; 16]) {
    for i in 0..16 { a[i] ^= b[i]; }
}

fn sub_bytes(state: &mut [u8; 16]) {
    for b in state.iter_mut() { *b = SBOX[*b as usize]; }
}

/// ShiftRows: row i is shifted left by i positions
/// State layout (column-major):
///   [0, 4, 8, 12]   row 0 — no shift
///   [1, 5, 9, 13]   row 1 — shift left 1
///   [2, 6, 10, 14]  row 2 — shift left 2
///   [3, 7, 11, 15]  row 3 — shift left 3
fn shift_rows(s: &mut [u8; 16]) {
    // Row 1
    let t = s[1];
    s[1] = s[5]; s[5] = s[9]; s[9] = s[13]; s[13] = t;
    // Row 2
    let (t0, t1) = (s[2], s[6]);
    s[2] = s[10]; s[6] = s[14]; s[10] = t0; s[14] = t1;
    // Row 3
    let t = s[15];
    s[15] = s[11]; s[11] = s[7]; s[7] = s[3]; s[3] = t;
}

fn xtime(a: u8) -> u8 {
    (a << 1) ^ (if a & 0x80 != 0 { 0x1b } else { 0x00 })
}

/// MixColumns: each column multiplied by fixed polynomial
fn mix_columns(s: &mut [u8; 16]) {
    for col in 0..4 {
        let i = col * 4;
        let (a0, a1, a2, a3) = (s[i], s[i + 1], s[i + 2], s[i + 3]);
        let t = a0 ^ a1 ^ a2 ^ a3;
        s[i]     = a0 ^ xtime(a0 ^ a1) ^ t;
        s[i + 1] = a1 ^ xtime(a1 ^ a2) ^ t;
        s[i + 2] = a2 ^ xtime(a2 ^ a3) ^ t;
        s[i + 3] = a3 ^ xtime(a3 ^ a0) ^ t;
    }
}
```

- [ ] **Step 4: Run AES test**

Run: `cargo test aes128_single_block 2>&1 | tail -5`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/core/net/tls/crypto/aes_gcm.rs tests/core/net/tls/crypto/aes_gcm_test.rs
git commit -m "feat(tls): implement AES-128 core with NIST test vector"
```

---

## Task 5: AES-128-GCM (GHASH + GCM)

**Files:**
- Modify: `src/core/net/tls/crypto/aes_gcm.rs`
- Test: `tests/core/net/tls/crypto/aes_gcm_test.rs`

- [ ] **Step 1: Write GCM encrypt/decrypt tests**

```rust
// append to tests/core/net/tls/crypto/aes_gcm_test.rs
use viv::core::net::tls::crypto::aes_gcm::Aes128Gcm;

/// GCM spec Test Case 2: 16-byte plaintext, empty AAD
#[test]
fn aes128_gcm_tc2_encrypt() {
    let key = [0u8; 16];
    let nonce = [0u8; 12];
    let plaintext = [0u8; 16];
    let gcm = Aes128Gcm::new(&key);

    let mut out = vec![0u8; 16 + 16]; // ciphertext + tag
    gcm.encrypt(&nonce, &[], &plaintext, &mut out);

    let expected_ct: [u8; 16] = [
        0x03, 0x88, 0xda, 0xce, 0x60, 0xb6, 0xa3, 0x92,
        0xf3, 0x28, 0xc2, 0xb9, 0x71, 0xb2, 0xfe, 0x78,
    ];
    let expected_tag: [u8; 16] = [
        0xab, 0x6e, 0x47, 0xd4, 0x2c, 0xec, 0x13, 0xbd,
        0xf5, 0x3a, 0x67, 0xb2, 0x12, 0x57, 0xbd, 0xdf,
    ];
    assert_eq!(&out[..16], &expected_ct);
    assert_eq!(&out[16..], &expected_tag);
}

/// GCM spec Test Case 2: decrypt
#[test]
fn aes128_gcm_tc2_decrypt() {
    let key = [0u8; 16];
    let nonce = [0u8; 12];
    let mut ciphertext_and_tag = vec![
        0x03, 0x88, 0xda, 0xce, 0x60, 0xb6, 0xa3, 0x92,
        0xf3, 0x28, 0xc2, 0xb9, 0x71, 0xb2, 0xfe, 0x78,
        0xab, 0x6e, 0x47, 0xd4, 0x2c, 0xec, 0x13, 0xbd,
        0xf5, 0x3a, 0x67, 0xb2, 0x12, 0x57, 0xbd, 0xdf,
    ];
    let gcm = Aes128Gcm::new(&key);
    let mut out = vec![0u8; 16];
    let n = gcm.decrypt(&nonce, &[], &ciphertext_and_tag, &mut out).unwrap();
    assert_eq!(n, 16);
    assert_eq!(out, vec![0u8; 16]);
}

/// GCM: decrypt with tampered tag should fail
#[test]
fn aes128_gcm_bad_tag() {
    let key = [0u8; 16];
    let nonce = [0u8; 12];
    let mut ciphertext_and_tag = vec![
        0x03, 0x88, 0xda, 0xce, 0x60, 0xb6, 0xa3, 0x92,
        0xf3, 0x28, 0xc2, 0xb9, 0x71, 0xb2, 0xfe, 0x78,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, // bad tag
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    ];
    let gcm = Aes128Gcm::new(&key);
    let mut out = vec![0u8; 16];
    assert!(gcm.decrypt(&nonce, &[], &ciphertext_and_tag, &mut out).is_err());
}

/// GCM spec Test Case 1: empty plaintext
#[test]
fn aes128_gcm_tc1_empty() {
    let key = [0u8; 16];
    let nonce = [0u8; 12];
    let gcm = Aes128Gcm::new(&key);

    let mut out = vec![0u8; 16]; // tag only
    gcm.encrypt(&nonce, &[], &[], &mut out);

    let expected_tag: [u8; 16] = [
        0x58, 0xe2, 0xfc, 0xce, 0xfa, 0x7e, 0x30, 0x61,
        0x36, 0x7f, 0x1d, 0x57, 0xa4, 0xe7, 0x45, 0x5a,
    ];
    assert_eq!(&out[..16], &expected_tag);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test aes128_gcm 2>&1 | tail -5`
Expected: FAIL — `Aes128Gcm` not found

- [ ] **Step 3: Implement GHASH + GCM**

Append to `src/core/net/tls/crypto/aes_gcm.rs`:

```rust
/// GF(2^128) multiplication for GHASH
/// Reduction polynomial: x^128 + x^7 + x^2 + x + 1
fn gf128_mul(x: &[u8; 16], y: &[u8; 16]) -> [u8; 16] {
    let mut z = [0u8; 16];
    let mut v = *x;

    for i in 0..128 {
        if (y[i / 8] >> (7 - (i % 8))) & 1 == 1 {
            for k in 0..16 { z[k] ^= v[k]; }
        }
        let lsb = v[15] & 1;
        for k in (1..16).rev() {
            v[k] = (v[k] >> 1) | (v[k - 1] << 7);
        }
        v[0] >>= 1;
        if lsb == 1 {
            v[0] ^= 0xe1;
        }
    }
    z
}

/// GHASH: iterative multiply-accumulate over 16-byte blocks
fn ghash(h: &[u8; 16], data: &[u8]) -> [u8; 16] {
    let mut y = [0u8; 16];
    let mut offset = 0;
    while offset < data.len() {
        let mut block = [0u8; 16];
        let end = (offset + 16).min(data.len());
        block[..end - offset].copy_from_slice(&data[offset..end]);
        for k in 0..16 { y[k] ^= block[k]; }
        y = gf128_mul(&y, h);
        offset += 16;
    }
    y
}

pub struct Aes128Gcm {
    aes: Aes128,
    h: [u8; 16], // GHASH subkey = AES(key, 0)
}

impl Aes128Gcm {
    pub fn new(key: &[u8; 16]) -> Self {
        let aes = Aes128::new(key);
        let h = aes.encrypt_block(&[0u8; 16]);
        Self { aes, h }
    }

    /// Encrypt: out must be plaintext.len() + 16 (tag) bytes
    pub fn encrypt(&self, nonce: &[u8; 12], aad: &[u8], plaintext: &[u8], out: &mut [u8]) {
        // J0 = nonce || 0x00000001
        let mut j0 = [0u8; 16];
        j0[..12].copy_from_slice(nonce);
        j0[15] = 1;

        // Encrypt plaintext with AES-CTR (starting from J0+1)
        let ct_len = plaintext.len();
        let mut counter = j0;
        for i in 0..((ct_len + 15) / 16) {
            inc32(&mut counter);
            let keystream = self.aes.encrypt_block(&counter);
            let start = i * 16;
            let end = (start + 16).min(ct_len);
            for j in start..end {
                out[j] = plaintext[j] ^ keystream[j - start];
            }
        }

        // Build GHASH input: pad(AAD) || pad(ciphertext) || len_block
        let mut ghash_input = Vec::new();
        ghash_input.extend_from_slice(aad);
        // Pad AAD to 16-byte boundary
        if aad.len() % 16 != 0 {
            ghash_input.resize(((aad.len() + 15) / 16) * 16, 0);
        }
        ghash_input.extend_from_slice(&out[..ct_len]);
        if ct_len % 16 != 0 {
            ghash_input.resize(ghash_input.len() + (16 - ct_len % 16), 0);
        }
        // Length block: 64-bit AAD bit len || 64-bit ciphertext bit len
        ghash_input.extend_from_slice(&((aad.len() as u64 * 8).to_be_bytes()));
        ghash_input.extend_from_slice(&((ct_len as u64 * 8).to_be_bytes()));

        let s = ghash(&self.h, &ghash_input);

        // Tag = GHASH_result XOR AES(K, J0)
        let encrypted_j0 = self.aes.encrypt_block(&j0);
        for i in 0..16 {
            out[ct_len + i] = s[i] ^ encrypted_j0[i];
        }
    }

    /// Decrypt: ciphertext_and_tag is ciphertext || 16-byte tag.
    /// out must be at least ciphertext_and_tag.len() - 16 bytes.
    /// Returns plaintext length on success.
    pub fn decrypt(
        &self, nonce: &[u8; 12], aad: &[u8],
        ciphertext_and_tag: &[u8], out: &mut [u8],
    ) -> crate::Result<usize> {
        if ciphertext_and_tag.len() < 16 {
            return Err(crate::Error::Tls("GCM: input too short".into()));
        }
        let ct_len = ciphertext_and_tag.len() - 16;
        let ct = &ciphertext_and_tag[..ct_len];
        let tag = &ciphertext_and_tag[ct_len..];

        // Verify tag first
        let mut expected_tag_buf = vec![0u8; ct_len + 16];
        expected_tag_buf[..ct_len].copy_from_slice(ct);
        // Recompute tag using encrypt on ciphertext (since CTR is symmetric)
        // Actually: recompute GHASH over ciphertext, then XOR with AES(K, J0)
        let mut j0 = [0u8; 16];
        j0[..12].copy_from_slice(nonce);
        j0[15] = 1;

        let mut ghash_input = Vec::new();
        ghash_input.extend_from_slice(aad);
        if aad.len() % 16 != 0 {
            ghash_input.resize(((aad.len() + 15) / 16) * 16, 0);
        }
        ghash_input.extend_from_slice(ct);
        if ct_len % 16 != 0 {
            ghash_input.resize(ghash_input.len() + (16 - ct_len % 16), 0);
        }
        ghash_input.extend_from_slice(&((aad.len() as u64 * 8).to_be_bytes()));
        ghash_input.extend_from_slice(&((ct_len as u64 * 8).to_be_bytes()));

        let s = ghash(&self.h, &ghash_input);
        let encrypted_j0 = self.aes.encrypt_block(&j0);
        let mut computed_tag = [0u8; 16];
        for i in 0..16 {
            computed_tag[i] = s[i] ^ encrypted_j0[i];
        }

        // Constant-time compare
        let mut diff = 0u8;
        for i in 0..16 { diff |= computed_tag[i] ^ tag[i]; }
        if diff != 0 {
            return Err(crate::Error::Tls("GCM: authentication failed".into()));
        }

        // Decrypt with CTR
        let mut counter = j0;
        for i in 0..((ct_len + 15) / 16) {
            inc32(&mut counter);
            let keystream = self.aes.encrypt_block(&counter);
            let start = i * 16;
            let end = (start + 16).min(ct_len);
            for j in start..end {
                out[j] = ct[j] ^ keystream[j - start];
            }
        }

        Ok(ct_len)
    }
}

/// Increment the rightmost 32 bits of a 16-byte counter (big-endian)
fn inc32(counter: &mut [u8; 16]) {
    for i in (12..16).rev() {
        counter[i] = counter[i].wrapping_add(1);
        if counter[i] != 0 { break; }
    }
}
```

- [ ] **Step 4: Run GCM tests**

Run: `cargo test aes128_gcm 2>&1 | tail -10`
Expected: All 4 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/core/net/tls/crypto/aes_gcm.rs tests/core/net/tls/crypto/aes_gcm_test.rs
git commit -m "feat(tls): implement AES-128-GCM with NIST test vectors"
```

---

## Task 6: X25519

**Files:**
- Create: `src/core/net/tls/crypto/x25519.rs`
- Test: `tests/core/net/tls/crypto/x25519_test.rs`

**Reference:** RFC 7748, `/data/dlab/rustls-ref/` for API style.

X25519 is the most math-heavy crypto primitive. Key concepts:
- Field: GF(2^255 - 19), elements represented as 5 × 51-bit limbs in `[u64; 5]`
- Curve point operations: Montgomery ladder (`x_only` — no y coordinate needed)
- Scalar multiplication: iterate over scalar bits, constant-time swap

- [ ] **Step 1: Write X25519 tests from RFC 7748**

```rust
// tests/core/net/tls/crypto/x25519_test.rs
use viv::core::net::tls::crypto::x25519;

/// RFC 7748 §5.2 Test Vector 1
#[test]
fn x25519_scalar_mul_tv1() {
    let scalar: [u8; 32] = [
        0xa5, 0x46, 0xe3, 0x6b, 0xf0, 0x52, 0x7c, 0x9d,
        0x3b, 0x16, 0x15, 0x4b, 0x82, 0x46, 0x5e, 0xdd,
        0x62, 0x14, 0x4c, 0x0a, 0xc1, 0xfc, 0x5a, 0x18,
        0x50, 0x6a, 0x22, 0x44, 0xba, 0x44, 0x9a, 0xc4,
    ];
    let u_in: [u8; 32] = [
        0xe6, 0xdb, 0x68, 0x67, 0x58, 0x30, 0x30, 0xdb,
        0x35, 0x94, 0xc1, 0xa4, 0x24, 0xb1, 0x5f, 0x7c,
        0x72, 0x66, 0x24, 0xec, 0x26, 0xb3, 0x35, 0x3b,
        0x10, 0xa9, 0x03, 0xa6, 0xd0, 0xab, 0x1c, 0x4c,
    ];
    let expected: [u8; 32] = [
        0xc3, 0xda, 0x55, 0x37, 0x9d, 0xe9, 0xc6, 0x90,
        0x8e, 0x94, 0xea, 0x4d, 0xf2, 0x8d, 0x08, 0x4f,
        0x32, 0xec, 0xcf, 0x03, 0x49, 0x1c, 0x71, 0xf7,
        0x54, 0xb4, 0x07, 0x55, 0x77, 0xa2, 0x85, 0x52,
    ];
    assert_eq!(x25519::scalarmult(&scalar, &u_in), expected);
}

/// RFC 7748 §6.1: DH key agreement (Alice + Bob)
#[test]
fn x25519_dh_rfc7748() {
    let alice_priv: [u8; 32] = [
        0x77, 0x07, 0x6d, 0x0a, 0x73, 0x18, 0xa5, 0x7d,
        0x3c, 0x16, 0xc1, 0x72, 0x51, 0xb2, 0x66, 0x45,
        0xdf, 0x4c, 0x2f, 0x87, 0xeb, 0xc0, 0x99, 0x2a,
        0xb1, 0x77, 0xfb, 0xa5, 0x1d, 0xb9, 0x2c, 0x2a,
    ];
    let alice_pub: [u8; 32] = [
        0x85, 0x20, 0xf0, 0x09, 0x89, 0x30, 0xa7, 0x54,
        0x74, 0x8b, 0x7d, 0xdc, 0xb4, 0x3e, 0xf7, 0x5a,
        0x0d, 0xbf, 0x3a, 0x0d, 0x26, 0x38, 0x1a, 0xf4,
        0xeb, 0xa4, 0xa9, 0x8e, 0xaa, 0x9b, 0x4e, 0x6a,
    ];
    let bob_priv: [u8; 32] = [
        0x5d, 0xab, 0x08, 0x7e, 0x62, 0x4a, 0x8a, 0x4b,
        0x79, 0xe1, 0x7f, 0x8b, 0x83, 0x80, 0x0e, 0xe6,
        0x6f, 0x3b, 0xb1, 0x29, 0x26, 0x18, 0xb6, 0xfd,
        0x1c, 0x2f, 0x8b, 0x27, 0xff, 0x88, 0xe0, 0xeb,
    ];
    let bob_pub: [u8; 32] = [
        0xde, 0x9e, 0xdb, 0x7d, 0x7b, 0x7d, 0xc1, 0xb4,
        0xd3, 0x5b, 0x61, 0xc2, 0xec, 0xe4, 0x35, 0x37,
        0x3f, 0x83, 0x43, 0xc8, 0x5b, 0x78, 0x67, 0x4d,
        0xad, 0xfc, 0x7e, 0x14, 0x6f, 0x88, 0x2b, 0x4f,
    ];
    let shared: [u8; 32] = [
        0x4a, 0x5d, 0x9d, 0x5b, 0xa4, 0xce, 0x2d, 0xe1,
        0x72, 0x8e, 0x3b, 0xf4, 0x80, 0x35, 0x0f, 0x25,
        0xe0, 0x7e, 0x21, 0xc9, 0x47, 0xd1, 0x9e, 0x33,
        0x76, 0xf0, 0x9b, 0x3c, 0x1e, 0x16, 0x17, 0x42,
    ];

    // Base point = 9
    let base_point = {
        let mut bp = [0u8; 32];
        bp[0] = 9;
        bp
    };

    // Verify public key derivation
    assert_eq!(x25519::scalarmult(&alice_priv, &base_point), alice_pub);
    assert_eq!(x25519::scalarmult(&bob_priv, &base_point), bob_pub);

    // Verify shared secret
    assert_eq!(x25519::scalarmult(&alice_priv, &bob_pub), shared);
    assert_eq!(x25519::scalarmult(&bob_priv, &alice_pub), shared);
}

/// keypair() generates valid keys
#[test]
fn x25519_keypair_works() {
    let (secret, public) = x25519::keypair().unwrap();
    let base_point = {
        let mut bp = [0u8; 32];
        bp[0] = 9;
        bp
    };
    assert_eq!(x25519::scalarmult(&secret, &base_point), public);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test x25519_ 2>&1 | tail -5`
Expected: FAIL

- [ ] **Step 3: Implement X25519**

Write `src/core/net/tls/crypto/x25519.rs`. This is the most complex primitive (~300 lines).

Key implementation structure:
- `Fe` type: `[u64; 5]` — field element in radix 2^51
- `fe_add`, `fe_sub`, `fe_mul`, `fe_sq`: field arithmetic
- `fe_invert`: compute inverse via Fermat's little theorem (a^(p-2))
- `fe_tobytes`, `fe_frombytes`: conversion to/from 32-byte little-endian
- `scalarmult`: Montgomery ladder — iterate 255 bits of scalar, constant-time conditional swap, doubling and differential addition
- `scalarmult_base`: multiply by base point (u=9)
- `keypair`: getrandom → clamp scalar → scalarmult_base

Refer to `/data/dlab/rustls-ref/` for ring's X25519 approach, and RFC 7748 §5 for the Montgomery ladder pseudocode. The clamping procedure (RFC 7748 §5):
```rust
fn clamp(scalar: &mut [u8; 32]) {
    scalar[0] &= 248;   // clear bottom 3 bits
    scalar[31] &= 127;  // clear top bit
    scalar[31] |= 64;   // set second-to-top bit
}
```

The field element multiplication is the hot path — use the schoolbook method with 128-bit intermediates (`u128`) to avoid overflow, then reduce mod 2^255-19.

- [ ] **Step 4: Run X25519 tests**

Run: `cargo test x25519_ 2>&1 | tail -10`
Expected: All 3 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/core/net/tls/crypto/x25519.rs tests/core/net/tls/crypto/x25519_test.rs
git commit -m "feat(tls): implement X25519 ECDHE with RFC 7748 test vectors"
```

---

## Task 7: Key Schedule

**Files:**
- Create: `src/core/net/tls/key_schedule.rs`
- Test: `tests/core/net/tls/key_schedule_test.rs`

**Reference:** RFC 8446 §7.1, RFC 8448 §3 test vectors.

- [ ] **Step 1: Write key schedule tests using RFC 8448 vectors**

```rust
// tests/core/net/tls/key_schedule_test.rs
use viv::core::net::tls::key_schedule::{hkdf_expand_label, KeySchedule};
use viv::core::net::tls::crypto::sha256::Sha256;

/// Test hkdf_expand_label with known TLS 1.3 derivation
/// Derive-Secret(early_secret, "derived", "") should produce a known value
#[test]
fn hkdf_expand_label_derived() {
    // Early secret (HKDF-Extract(0, 0))
    let early_secret: [u8; 32] = [
        0x33, 0xad, 0x0a, 0x1c, 0x60, 0x7e, 0xc0, 0x3b,
        0x09, 0xe6, 0xcd, 0x98, 0x93, 0x68, 0x0c, 0xe2,
        0x10, 0xad, 0xf3, 0x00, 0xaa, 0x1f, 0x26, 0x60,
        0xe1, 0xb2, 0x2e, 0x10, 0xf1, 0x70, 0xf9, 0x2a,
    ];

    // Derive-Secret(., "derived", "") = HKDF-Expand-Label(., "derived", Hash(""), 32)
    let empty_hash = Sha256::hash(b"");
    let mut out = [0u8; 32];
    hkdf_expand_label(&early_secret, b"derived", &empty_hash, &mut out);

    let expected: [u8; 32] = [
        0x6f, 0x26, 0x15, 0xa1, 0x08, 0xc7, 0x02, 0xc5,
        0x67, 0x8f, 0x54, 0xfc, 0x9d, 0xba, 0xb6, 0x97,
        0x16, 0xc0, 0x76, 0x18, 0x9c, 0x48, 0x25, 0x0c,
        0xeb, 0xea, 0xc3, 0x57, 0x6c, 0x36, 0x11, 0xba,
    ];
    assert_eq!(out, expected);
}

/// Full key schedule: early → handshake → master, verify intermediate values
#[test]
fn key_schedule_full_derivation() {
    let ecdhe_shared: [u8; 32] = [
        0x8b, 0xd4, 0x05, 0x4f, 0xb5, 0x5b, 0x9d, 0x63,
        0xfd, 0xfb, 0xac, 0xf9, 0xf0, 0x4b, 0x9f, 0x0d,
        0x35, 0xe6, 0xd6, 0x3f, 0x53, 0x75, 0x63, 0xef,
        0xd4, 0x62, 0x72, 0x90, 0x0f, 0x89, 0x49, 0x2d,
    ];

    // hello_hash = SHA-256(ClientHello || ServerHello) from RFC 8448
    let hello_hash: [u8; 32] = [
        0x86, 0x0c, 0x06, 0xed, 0xc0, 0x78, 0x58, 0xee,
        0x8e, 0x78, 0xf0, 0xe7, 0x42, 0x8c, 0x58, 0xed,
        0xd6, 0xb4, 0x3f, 0x2c, 0xa3, 0xe6, 0xe9, 0x5f,
        0x02, 0xed, 0x06, 0x3c, 0xf0, 0xe1, 0xca, 0xd8,
    ];

    let mut ks = KeySchedule::new();
    let (client_hs, server_hs) = ks.derive_handshake_secrets(&ecdhe_shared, &hello_hash);

    // Verify server handshake key
    assert_eq!(server_hs.key, [
        0x3f, 0xce, 0x51, 0x60, 0x09, 0xc2, 0x17, 0x27,
        0xd0, 0xf2, 0xe4, 0xe8, 0x6e, 0xe4, 0x03, 0xbc,
    ]);
    // Verify server handshake IV
    assert_eq!(server_hs.iv, [
        0x5d, 0x31, 0x3e, 0xb2, 0x67, 0x12, 0x76, 0xee,
        0x13, 0x00, 0x0b, 0x30,
    ]);
    // Verify client handshake key
    assert_eq!(client_hs.key, [
        0xdb, 0xfa, 0xa6, 0x93, 0xd1, 0x76, 0x2c, 0x5b,
        0x66, 0x6a, 0xf5, 0xd9, 0x50, 0x25, 0x8d, 0x01,
    ]);
    // Verify client handshake IV
    assert_eq!(client_hs.iv, [
        0x5b, 0xd3, 0xc7, 0x1b, 0x83, 0x6e, 0x0b, 0x76,
        0xbb, 0x73, 0x26, 0x5f,
    ]);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test key_schedule 2>&1 | tail -5`
Expected: FAIL

- [ ] **Step 3: Implement key schedule**

Write `src/core/net/tls/key_schedule.rs`:

```rust
use super::crypto::sha256::{Sha256, hmac_sha256, hkdf_extract, hkdf_expand};

pub struct TrafficKeys {
    pub key: [u8; 16],   // AES-128 key
    pub iv: [u8; 12],    // GCM nonce
}

pub struct KeySchedule {
    early_secret: [u8; 32],
    handshake_secret: [u8; 32],
    master_secret: [u8; 32],
    client_hs_secret: [u8; 32],
    server_hs_secret: [u8; 32],
}

impl KeySchedule {
    /// Phase 1: compute Early Secret from zero (no PSK)
    pub fn new() -> Self {
        let zeros = [0u8; 32];
        let early_secret = hkdf_extract(&zeros, &zeros);
        Self {
            early_secret,
            handshake_secret: [0; 32],
            master_secret: [0; 32],
            client_hs_secret: [0; 32],
            server_hs_secret: [0; 32],
        }
    }

    /// Phase 2: input ECDHE shared secret + Hash(CH..SH)
    pub fn derive_handshake_secrets(
        &mut self,
        shared_secret: &[u8; 32],
        hello_hash: &[u8; 32],
    ) -> (TrafficKeys, TrafficKeys) {
        let empty_hash = Sha256::hash(b"");
        let derived = derive_secret(&self.early_secret, b"derived", &empty_hash);
        self.handshake_secret = hkdf_extract(&derived, shared_secret);

        self.client_hs_secret = derive_secret(&self.handshake_secret, b"c hs traffic", hello_hash);
        self.server_hs_secret = derive_secret(&self.handshake_secret, b"s hs traffic", hello_hash);

        (
            traffic_keys(&self.client_hs_secret),
            traffic_keys(&self.server_hs_secret),
        )
    }

    /// Phase 3: input Hash(CH..ServerFinished) → application keys
    pub fn derive_app_secrets(
        &mut self,
        handshake_hash: &[u8; 32],
    ) -> (TrafficKeys, TrafficKeys) {
        let empty_hash = Sha256::hash(b"");
        let derived = derive_secret(&self.handshake_secret, b"derived", &empty_hash);
        let zeros = [0u8; 32];
        self.master_secret = hkdf_extract(&derived, &zeros);

        let client_secret = derive_secret(&self.master_secret, b"c ap traffic", handshake_hash);
        let server_secret = derive_secret(&self.master_secret, b"s ap traffic", handshake_hash);

        (traffic_keys(&client_secret), traffic_keys(&server_secret))
    }

    pub fn server_finished_key(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        hkdf_expand_label(&self.server_hs_secret, b"finished", &[], &mut out);
        out
    }

    pub fn client_finished_key(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        hkdf_expand_label(&self.client_hs_secret, b"finished", &[], &mut out);
        out
    }
}

fn traffic_keys(secret: &[u8; 32]) -> TrafficKeys {
    let mut key = [0u8; 16];
    let mut iv = [0u8; 12];
    hkdf_expand_label(secret, b"key", &[], &mut key);
    hkdf_expand_label(secret, b"iv", &[], &mut iv);
    TrafficKeys { key, iv }
}

fn derive_secret(secret: &[u8], label: &[u8], transcript_hash: &[u8; 32]) -> [u8; 32] {
    let mut out = [0u8; 32];
    hkdf_expand_label(secret, label, transcript_hash, &mut out);
    out
}

/// RFC 8446 HKDF-Expand-Label
/// info = length(2B) || len("tls13 "+label)(1B) || "tls13 " || label || len(context)(1B) || context
pub fn hkdf_expand_label(secret: &[u8], label: &[u8], context: &[u8], out: &mut [u8]) {
    let tls_label_len = 6 + label.len(); // "tls13 " + label
    let mut info = Vec::with_capacity(2 + 1 + tls_label_len + 1 + context.len());
    info.push((out.len() >> 8) as u8);
    info.push(out.len() as u8);
    info.push(tls_label_len as u8);
    info.extend_from_slice(b"tls13 ");
    info.extend_from_slice(label);
    info.push(context.len() as u8);
    info.extend_from_slice(context);

    hkdf_expand(secret, &info, out);
}
```

- [ ] **Step 4: Run key schedule tests**

Run: `cargo test key_schedule 2>&1 | tail -10`
Expected: All tests PASS

Note: If RFC 8448 vector tests fail, debug by printing intermediate values and comparing against the RFC. The most common bug is in `hkdf_expand_label` info encoding.

- [ ] **Step 5: Commit**

```bash
git add src/core/net/tls/key_schedule.rs tests/core/net/tls/key_schedule_test.rs
git commit -m "feat(tls): implement TLS 1.3 key schedule with RFC 8448 test vectors"
```

---

## Task 8: Codec — Message Encoding/Decoding

**Files:**
- Create: `src/core/net/tls/codec.rs`
- Test: `tests/core/net/tls/codec_test.rs`

- [ ] **Step 1: Write codec tests**

```rust
// tests/core/net/tls/codec_test.rs
use viv::core::net::tls::codec;

#[test]
fn encode_client_hello_starts_with_correct_header() {
    let random = [0x01u8; 32];
    let session_id = [0x02u8; 32];
    let x25519_pub = [0x03u8; 32];
    let mut out = Vec::new();
    codec::encode_client_hello(&random, &session_id, "example.com", &x25519_pub, &mut out);

    // Handshake header: type=0x01 (ClientHello), then 3-byte length
    assert_eq!(out[0], 0x01);
    let len = (out[1] as usize) << 16 | (out[2] as usize) << 8 | (out[3] as usize);
    assert_eq!(len, out.len() - 4); // length covers everything after the 4-byte header

    // legacy_version = 0x0303
    assert_eq!(out[4], 0x03);
    assert_eq!(out[5], 0x03);

    // random = 32 bytes of 0x01
    assert_eq!(&out[6..38], &[0x01u8; 32]);
}

#[test]
fn encode_finished_correct_format() {
    let verify_data = [0xabu8; 32];
    let mut out = Vec::new();
    codec::encode_finished(&verify_data, &mut out);

    // Handshake header: type=0x14 (Finished), length=32
    assert_eq!(out[0], 0x14);
    assert_eq!(out[1], 0x00);
    assert_eq!(out[2], 0x00);
    assert_eq!(out[3], 0x20); // 32
    assert_eq!(&out[4..36], &[0xabu8; 32]);
}

#[test]
fn decode_server_hello_extracts_key_share() {
    // Build a minimal ServerHello with known x25519 public key
    let x25519_pub = [0x42u8; 32];
    let server_hello = build_test_server_hello(&x25519_pub);
    let parsed = codec::decode_handshake(&server_hello).unwrap();
    match parsed {
        codec::HandshakeMessage::ServerHello(sh) => {
            assert_eq!(sh.x25519_public, x25519_pub);
            assert_eq!(sh.cipher_suite, 0x1301);
        }
        _ => panic!("expected ServerHello"),
    }
}

// Helper: build a minimal but valid ServerHello byte sequence
fn build_test_server_hello(x25519_pub: &[u8; 32]) -> Vec<u8> {
    let mut msg = Vec::new();

    // legacy_version
    msg.extend_from_slice(&[0x03, 0x03]);
    // random (32 bytes)
    msg.extend_from_slice(&[0xaa; 32]);
    // session_id_len + session_id (32 bytes)
    msg.push(32);
    msg.extend_from_slice(&[0xbb; 32]);
    // cipher suite
    msg.extend_from_slice(&[0x13, 0x01]); // TLS_AES_128_GCM_SHA256
    // compression
    msg.push(0x00);

    // Extensions
    let mut exts = Vec::new();

    // supported_versions extension (type=43, value=0x0304)
    exts.extend_from_slice(&[0x00, 0x2b]); // type
    exts.extend_from_slice(&[0x00, 0x02]); // len
    exts.extend_from_slice(&[0x03, 0x04]); // TLS 1.3

    // key_share extension (type=51)
    exts.extend_from_slice(&[0x00, 0x33]); // type
    let ks_data_len = 2 + 2 + 32; // group(2) + key_len(2) + key(32)
    exts.push((ks_data_len >> 8) as u8);
    exts.push(ks_data_len as u8);
    exts.extend_from_slice(&[0x00, 0x1d]); // x25519
    exts.extend_from_slice(&[0x00, 0x20]); // 32 bytes
    exts.extend_from_slice(x25519_pub);

    // extensions length
    msg.push((exts.len() >> 8) as u8);
    msg.push(exts.len() as u8);
    msg.extend_from_slice(&exts);

    // Wrap in handshake header
    let mut out = Vec::new();
    out.push(0x02); // ServerHello
    out.push((msg.len() >> 16) as u8);
    out.push((msg.len() >> 8) as u8);
    out.push(msg.len() as u8);
    out.extend_from_slice(&msg);
    out
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test codec 2>&1 | tail -5`
Expected: FAIL

- [ ] **Step 3: Implement codec**

Write `src/core/net/tls/codec.rs`. Key aspects:

- `encode_client_hello`: build complete ClientHello with extensions (server_name, supported_versions, supported_groups, key_share, signature_algorithms). All length-prefixed fields in big-endian.
- `encode_finished`: simple — handshake header + 32 bytes verify_data
- `encode_change_cipher_spec`: single byte `0x01`
- `decode_handshake`: parse handshake header (type + 3-byte length), dispatch by type
- `decode_server_hello`: parse legacy_version, random, session_id, cipher_suite, compression, then scan extensions for `key_share` (type=51) to extract x25519 public key and `supported_versions` (type=43) to confirm TLS 1.3
- `decode_encrypted_extensions`: skip content, just consume
- `decode_certificate`: store raw DER cert chain bytes
- `decode_certificate_verify`: store scheme + signature bytes
- `decode_finished`: read 32 bytes verify_data

All decoding uses a cursor (`&[u8]` + offset) for zero-copy parsing. Unknown extensions are skipped by reading their length and advancing the cursor.

- [ ] **Step 4: Run codec tests**

Run: `cargo test codec 2>&1 | tail -10`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/core/net/tls/codec.rs tests/core/net/tls/codec_test.rs
git commit -m "feat(tls): implement TLS message codec (encode/decode)"
```

---

## Task 9: Record Layer

**Files:**
- Create: `src/core/net/tls/record.rs`
- Test: `tests/core/net/tls/record_test.rs`

- [ ] **Step 1: Write record layer tests**

```rust
// tests/core/net/tls/record_test.rs
use viv::core::net::tls::record::RecordLayer;

#[test]
fn write_plaintext_record() {
    let rl = RecordLayer::new();
    let mut out = Vec::new();
    rl.write_plaintext(0x16, b"hello", &mut out); // Handshake content type

    // 5-byte header: content_type(1) + 0x0301(2) + length(2)
    assert_eq!(out[0], 0x16);
    assert_eq!(out[1], 0x03);
    assert_eq!(out[2], 0x01); // legacy record version for ClientHello
    assert_eq!(out[3], 0x00);
    assert_eq!(out[4], 0x05); // length = 5
    assert_eq!(&out[5..], b"hello");
}

#[test]
fn encrypt_then_decrypt_roundtrip() {
    let mut rl = RecordLayer::new();
    let key = [0x42u8; 16];
    let iv = [0x13u8; 12];
    rl.install_encrypter(key, iv);
    rl.install_decrypter(key, iv);

    // Encrypt
    let mut encrypted = Vec::new();
    rl.write_encrypted(0x17, b"secret data", &mut encrypted);

    // Header should say ApplicationData(0x17), version 0x0303
    assert_eq!(encrypted[0], 0x17);
    assert_eq!(encrypted[1], 0x03);
    assert_eq!(encrypted[2], 0x03);

    // Decrypt
    let (content_type, plaintext, consumed) = rl.read_record(&encrypted).unwrap();
    assert_eq!(content_type, 0x17);
    assert_eq!(plaintext, b"secret data");
    assert_eq!(consumed, encrypted.len());
}

#[test]
fn nonce_increments_per_record() {
    let mut rl = RecordLayer::new();
    let key = [0x01u8; 16];
    let iv = [0x00u8; 12];
    rl.install_encrypter(key, iv);
    rl.install_decrypter(key, iv);

    // Send two records — they must use different nonces (seq 0, seq 1)
    let mut enc1 = Vec::new();
    let mut enc2 = Vec::new();
    rl.write_encrypted(0x17, b"msg1", &mut enc1);
    rl.write_encrypted(0x17, b"msg2", &mut enc2);

    // Ciphertext should differ (different nonces)
    assert_ne!(enc1[5..], enc2[5..]);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test record 2>&1 | tail -5`
Expected: FAIL

- [ ] **Step 3: Implement record layer**

Write `src/core/net/tls/record.rs`:

```rust
use super::crypto::aes_gcm::Aes128Gcm;

pub struct RecordLayer {
    encrypter: Option<RecordEncrypter>,
    decrypter: Option<RecordDecrypter>,
}

struct RecordEncrypter {
    aes: Aes128Gcm,
    iv: [u8; 12],
    seq: u64,
}

struct RecordDecrypter {
    aes: Aes128Gcm,
    iv: [u8; 12],
    seq: u64,
}

impl RecordLayer {
    pub fn new() -> Self {
        Self { encrypter: None, decrypter: None }
    }

    pub fn install_encrypter(&mut self, key: [u8; 16], iv: [u8; 12]) {
        self.encrypter = Some(RecordEncrypter {
            aes: Aes128Gcm::new(&key),
            iv,
            seq: 0,
        });
    }

    pub fn install_decrypter(&mut self, key: [u8; 16], iv: [u8; 12]) {
        self.decrypter = Some(RecordDecrypter {
            aes: Aes128Gcm::new(&key),
            iv,
            seq: 0,
        });
    }

    /// Write a plaintext record (for ClientHello, before encryption is active)
    pub fn write_plaintext(&self, content_type: u8, payload: &[u8], out: &mut Vec<u8>) {
        out.push(content_type);
        out.extend_from_slice(&[0x03, 0x01]); // legacy version TLS 1.0 for first ClientHello
        out.push((payload.len() >> 8) as u8);
        out.push(payload.len() as u8);
        out.extend_from_slice(payload);
    }

    /// Write an encrypted record (TLS 1.3 format)
    pub fn write_encrypted(&mut self, content_type: u8, payload: &[u8], out: &mut Vec<u8>) {
        let enc = self.encrypter.as_mut().expect("encrypter not installed");
        let nonce = make_nonce(&enc.iv, enc.seq);
        enc.seq += 1;

        // TLS 1.3: inner plaintext = payload || content_type
        let mut inner = Vec::with_capacity(payload.len() + 1);
        inner.extend_from_slice(payload);
        inner.push(content_type);

        // Encrypt: output = ciphertext || tag (16 bytes)
        let encrypted_len = inner.len() + 16;
        let mut encrypted = vec![0u8; encrypted_len];

        // AAD = record header (5 bytes)
        let mut header = [0u8; 5];
        header[0] = 0x17; // outer content type = ApplicationData
        header[1] = 0x03;
        header[2] = 0x03; // legacy version TLS 1.2
        header[3] = (encrypted_len >> 8) as u8;
        header[4] = encrypted_len as u8;

        enc.aes.encrypt(&nonce, &header, &inner, &mut encrypted);

        out.extend_from_slice(&header);
        out.extend_from_slice(&encrypted);
    }

    /// Read one record from data. Returns (content_type, plaintext, bytes_consumed).
    pub fn read_record(&mut self, data: &[u8]) -> crate::Result<(u8, Vec<u8>, usize)> {
        if data.len() < 5 {
            return Err(crate::Error::Tls("record too short".into()));
        }
        let outer_type = data[0];
        let len = ((data[3] as usize) << 8) | (data[4] as usize);
        if data.len() < 5 + len {
            return Err(crate::Error::Tls("incomplete record".into()));
        }
        let payload = &data[5..5 + len];
        let consumed = 5 + len;

        if let Some(dec) = self.decrypter.as_mut() {
            // Decrypt
            let nonce = make_nonce(&dec.iv, dec.seq);
            dec.seq += 1;

            let mut plaintext = vec![0u8; len]; // more than enough
            let aad = &data[..5];
            let pt_len = dec.aes.decrypt(&nonce, aad, payload, &mut plaintext)?;

            // Strip trailing zeros and extract real content type (last non-zero byte)
            let mut real_len = pt_len;
            while real_len > 0 && plaintext[real_len - 1] == 0 {
                real_len -= 1;
            }
            if real_len == 0 {
                return Err(crate::Error::Tls("empty inner plaintext".into()));
            }
            let real_type = plaintext[real_len - 1];
            plaintext.truncate(real_len - 1);

            Ok((real_type, plaintext, consumed))
        } else {
            // No decrypter — return plaintext as-is
            Ok((outer_type, payload.to_vec(), consumed))
        }
    }
}

fn make_nonce(iv: &[u8; 12], seq: u64) -> [u8; 12] {
    let mut nonce = *iv;
    let seq_bytes = seq.to_be_bytes();
    // XOR sequence number into the last 8 bytes of IV
    for i in 0..8 {
        nonce[4 + i] ^= seq_bytes[i];
    }
    nonce
}
```

- [ ] **Step 4: Run record layer tests**

Run: `cargo test record 2>&1 | tail -10`
Expected: All 3 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/core/net/tls/record.rs tests/core/net/tls/record_test.rs
git commit -m "feat(tls): implement TLS 1.3 record layer with AEAD encrypt/decrypt"
```

---

## Task 10: Handshake State Machine

**Files:**
- Create: `src/core/net/tls/handshake.rs`
- Create: `src/core/net/tls/x509.rs`

This is the state machine that drives the TLS 1.3 handshake. It is mostly glue code connecting codec + key_schedule + record.

- [ ] **Step 1: Write x509 stub**

```rust
// src/core/net/tls/x509.rs
// Certificate verification stub. Currently accepts all certificates.
// TODO: implement X.509 chain validation in a future iteration.
```

- [ ] **Step 2: Implement handshake state machine**

Write `src/core/net/tls/handshake.rs`:

```rust
use super::codec::{self, HandshakeMessage};
use super::crypto::sha256::{Sha256, hmac_sha256};
use super::crypto::x25519;
use super::crypto::getrandom;
use super::key_schedule::KeySchedule;
use super::record::RecordLayer;

enum State {
    ExpectServerHello,
    ExpectEncryptedExtensions,
    ExpectCertOrCertReq,
    ExpectCertificateVerify,
    ExpectFinished,
    Complete,
}

pub struct Handshake {
    state: State,
    pub transcript: Sha256,
    pub key_schedule: KeySchedule,
    x25519_secret: [u8; 32],
    x25519_public: [u8; 32],
    server_name: String,
}

pub enum HandshakeResult {
    Continue,
    Complete,
}

impl Handshake {
    pub fn new(server_name: &str) -> crate::Result<Self> {
        let (secret, public) = x25519::keypair()?;
        Ok(Self {
            state: State::ExpectServerHello,
            transcript: Sha256::new(),
            key_schedule: KeySchedule::new(),
            x25519_secret: secret,
            x25519_public: public,
            server_name: server_name.into(),
        })
    }

    /// Build the ClientHello message bytes (handshake payload, no record header)
    pub fn encode_client_hello(&mut self) -> crate::Result<Vec<u8>> {
        let mut random = [0u8; 32];
        getrandom(&mut random)?;
        let mut session_id = [0u8; 32];
        getrandom(&mut session_id)?;

        let mut out = Vec::new();
        codec::encode_client_hello(
            &random, &session_id, &self.server_name,
            &self.x25519_public, &mut out,
        );
        self.transcript.update(&out);
        Ok(out)
    }

    /// Process a handshake message. Returns whether handshake is complete.
    pub fn handle_message(
        &mut self,
        msg_bytes: &[u8],
        record: &mut RecordLayer,
    ) -> crate::Result<HandshakeResult> {
        // Update transcript with raw handshake bytes
        self.transcript.update(msg_bytes);

        let msg = codec::decode_handshake(msg_bytes)?;

        match (&self.state, msg) {
            (State::ExpectServerHello, HandshakeMessage::ServerHello(sh)) => {
                // Verify TLS 1.3 and our cipher suite
                if sh.cipher_suite != 0x1301 {
                    return Err(crate::Error::Tls(
                        format!("unsupported cipher suite: 0x{:04x}", sh.cipher_suite),
                    ));
                }

                // Complete ECDHE
                let shared = x25519::shared_secret(&self.x25519_secret, &sh.x25519_public);

                // Derive handshake traffic keys
                let hello_hash = self.transcript.clone().finish();
                let (client_hs, server_hs) =
                    self.key_schedule.derive_handshake_secrets(&shared, &hello_hash);

                // Install server handshake decrypter
                record.install_decrypter(server_hs.key, server_hs.iv);
                // Install client handshake encrypter (for Finished message)
                record.install_encrypter(client_hs.key, client_hs.iv);

                self.state = State::ExpectEncryptedExtensions;
                Ok(HandshakeResult::Continue)
            }

            (State::ExpectEncryptedExtensions, HandshakeMessage::EncryptedExtensions(_)) => {
                self.state = State::ExpectCertOrCertReq;
                Ok(HandshakeResult::Continue)
            }

            (State::ExpectCertOrCertReq, HandshakeMessage::Certificate(_cert)) => {
                // TODO: verify certificate chain
                self.state = State::ExpectCertificateVerify;
                Ok(HandshakeResult::Continue)
            }

            (State::ExpectCertificateVerify, HandshakeMessage::CertificateVerify(_cv)) => {
                // TODO: verify signature
                self.state = State::ExpectFinished;
                Ok(HandshakeResult::Continue)
            }

            (State::ExpectFinished, HandshakeMessage::Finished(fin)) => {
                // Verify server Finished
                let finished_key = self.key_schedule.server_finished_key();
                // transcript hash up to (but not including) Finished
                // We already updated transcript above, so we need the hash BEFORE this message.
                // Fix: transcript should be updated AFTER verification for Finished.
                // This requires adjusting the flow — see note below.
                let expected = hmac_sha256(&finished_key, &self.get_pre_finished_hash());
                if expected != fin.verify_data {
                    return Err(crate::Error::Tls("server Finished verification failed".into()));
                }

                self.state = State::Complete;
                Ok(HandshakeResult::Complete)
            }

            _ => Err(crate::Error::Tls(
                format!("unexpected message in state {:?}", self.state_name()),
            )),
        }
    }

    /// Build the client Finished message
    pub fn encode_client_finished(&self) -> Vec<u8> {
        let finished_key = self.key_schedule.client_finished_key();
        let transcript_hash = self.transcript.clone().finish();
        let verify_data = hmac_sha256(&finished_key, &transcript_hash);
        let mut out = Vec::new();
        codec::encode_finished(&verify_data, &mut out);
        out
    }

    /// Switch record layer to application traffic keys
    pub fn install_app_keys(&mut self, record: &mut RecordLayer) {
        let transcript_hash = self.transcript.clone().finish();
        let (client_app, server_app) =
            self.key_schedule.derive_app_secrets(&transcript_hash);
        record.install_decrypter(server_app.key, server_app.iv);
        record.install_encrypter(client_app.key, client_app.iv);
    }

    fn state_name(&self) -> &str {
        match self.state {
            State::ExpectServerHello => "ExpectServerHello",
            State::ExpectEncryptedExtensions => "ExpectEncryptedExtensions",
            State::ExpectCertOrCertReq => "ExpectCertOrCertReq",
            State::ExpectCertificateVerify => "ExpectCertificateVerify",
            State::ExpectFinished => "ExpectFinished",
            State::Complete => "Complete",
        }
    }
}
```

**Important note on transcript and Finished verification:** The `handle_message` method updates the transcript BEFORE processing. For the Finished message, we need the transcript hash **before** the Finished message is included. The implementation needs to take a snapshot of the transcript hash before calling `self.transcript.update(msg_bytes)` for the Finished state. Adjust the flow:

```rust
// In handle_message, for ExpectFinished specifically:
// 1. Take transcript hash BEFORE update
// 2. Verify Finished
// 3. Then update transcript

// The cleanest approach: don't auto-update transcript at the top of handle_message.
// Instead, let each state handler update transcript at the right time.
```

This is a correctness detail the implementer must get right. The transcript hash for server Finished verification must exclude the Finished message itself, but the transcript for client Finished and app keys must include it.

- [ ] **Step 3: Verify compilation**

Run: `cargo build 2>&1 | tail -10`
Expected: Compiles (handshake is not wired to TlsStream yet)

- [ ] **Step 4: Commit**

```bash
git add src/core/net/tls/handshake.rs src/core/net/tls/x509.rs
git commit -m "feat(tls): implement TLS 1.3 handshake state machine"
```

---

## Task 11: TlsStream + AsyncTlsStream Integration

**Files:**
- Modify: `src/core/net/tls/mod.rs`

This wires everything together into the public API.

- [ ] **Step 1: Implement synchronous TlsStream**

Write the full `src/core/net/tls/mod.rs`:

```rust
pub mod crypto;
pub mod key_schedule;
pub mod codec;
pub mod record;
pub mod handshake;
pub mod x509;

use std::io::{self, Read, Write};
use record::RecordLayer;
use handshake::{Handshake, HandshakeResult};
use super::tcp;

pub struct TlsStream {
    tcp: std::net::TcpStream,
    record: RecordLayer,
    read_buf: Vec<u8>,
    plaintext_buf: Vec<u8>,
}

impl TlsStream {
    pub fn connect(host: &str, port: u16) -> crate::Result<Self> {
        let mut tcp = tcp::connect(host, port)?;
        let mut record = RecordLayer::new();
        let mut hs = Handshake::new(host)?;

        // 1. Send ClientHello
        let ch = hs.encode_client_hello()?;
        let mut out = Vec::new();
        record.write_plaintext(0x16, &ch, &mut out);
        tcp.write_all(&out)?;

        // 2. Read and process handshake messages until complete
        let mut buf = vec![0u8; 16384 + 256];
        let mut pending = Vec::new();
        let mut sent_ccs = false;

        loop {
            let n = tcp.read(&mut buf)?;
            if n == 0 {
                return Err(crate::Error::Tls("connection closed during handshake".into()));
            }
            pending.extend_from_slice(&buf[..n]);

            while pending.len() >= 5 {
                let rec_len = ((pending[3] as usize) << 8) | (pending[4] as usize);
                if pending.len() < 5 + rec_len {
                    break; // need more data
                }

                let outer_type = pending[0];

                // Skip ChangeCipherSpec from server
                if outer_type == 0x14 {
                    pending.drain(..5 + rec_len);
                    continue;
                }

                let (content_type, payload, consumed) = record.read_record(&pending)?;
                pending.drain(..consumed);

                // Handshake messages may be coalesced in a single record
                let mut hs_offset = 0;
                while hs_offset < payload.len() {
                    if payload.len() - hs_offset < 4 {
                        break;
                    }
                    let msg_len = ((payload[hs_offset + 1] as usize) << 16)
                        | ((payload[hs_offset + 2] as usize) << 8)
                        | (payload[hs_offset + 3] as usize);
                    let msg_end = hs_offset + 4 + msg_len;
                    if msg_end > payload.len() {
                        break;
                    }
                    let msg_bytes = &payload[hs_offset..msg_end];
                    let result = hs.handle_message(msg_bytes, &mut record)?;

                    if let HandshakeResult::Complete = result {
                        // Send ChangeCipherSpec (middlebox compat)
                        if !sent_ccs {
                            let mut ccs_out = Vec::new();
                            record.write_plaintext(0x14, &[0x01], &mut ccs_out);
                            // Fix version for CCS
                            ccs_out[1] = 0x03;
                            ccs_out[2] = 0x03;
                            tcp.write_all(&ccs_out)?;
                            sent_ccs = true;
                        }

                        // Send client Finished (encrypted with handshake key)
                        let fin = hs.encode_client_finished();
                        let mut fin_out = Vec::new();
                        record.write_encrypted(0x16, &fin, &mut fin_out);
                        tcp.write_all(&fin_out)?;

                        // Switch to application keys
                        hs.install_app_keys(&mut record);

                        return Ok(TlsStream {
                            tcp,
                            record,
                            read_buf: Vec::with_capacity(16384 + 256),
                            plaintext_buf: Vec::new(),
                        });
                    }

                    hs_offset = msg_end;
                }
            }
        }
    }
}

impl Read for TlsStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if !self.plaintext_buf.is_empty() {
            let n = buf.len().min(self.plaintext_buf.len());
            buf[..n].copy_from_slice(&self.plaintext_buf[..n]);
            self.plaintext_buf.drain(..n);
            return Ok(n);
        }

        // Read records from TCP until we get application data
        loop {
            let mut tcp_buf = vec![0u8; 16384 + 256];
            let n = self.tcp.read(&mut tcp_buf)?;
            if n == 0 { return Ok(0); }
            self.read_buf.extend_from_slice(&tcp_buf[..n]);

            while self.read_buf.len() >= 5 {
                let rec_len = ((self.read_buf[3] as usize) << 8)
                    | (self.read_buf[4] as usize);
                if self.read_buf.len() < 5 + rec_len {
                    break;
                }

                let (content_type, payload, consumed) = self.record.read_record(&self.read_buf)
                    .map_err(|e| io::Error::other(e.to_string()))?;
                self.read_buf.drain(..consumed);

                if content_type == 0x17 {
                    // Application data
                    let n = buf.len().min(payload.len());
                    buf[..n].copy_from_slice(&payload[..n]);
                    if n < payload.len() {
                        self.plaintext_buf.extend_from_slice(&payload[n..]);
                    }
                    return Ok(n);
                }
                // Skip alerts, other types for now
            }
        }
    }
}

impl Write for TlsStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut out = Vec::new();
        // Fragment into ≤16KB chunks
        for chunk in buf.chunks(16384) {
            self.record.write_encrypted(0x17, chunk, &mut out);
        }
        self.tcp.write_all(&out)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.tcp.flush()
    }
}

impl Drop for TlsStream {
    fn drop(&mut self) {
        // Send close_notify (best-effort)
        let mut out = Vec::new();
        self.record.write_encrypted(0x15, &[1, 0], &mut out);
        let _ = self.tcp.write_all(&out);
    }
}
```

- [ ] **Step 2: Implement AsyncTlsStream**

Add to `src/core/net/tls/mod.rs` — same structure, using `AsyncTcpStream` and `await`:

```rust
use super::async_tcp::AsyncTcpStream;
use super::async_tls::{wait_readable, wait_writable};
use std::os::unix::io::AsRawFd;

pub struct AsyncTlsStream {
    tcp: AsyncTcpStream,
    record: RecordLayer,
    read_buf: Vec<u8>,
    plaintext_buf: Vec<u8>,
}

impl AsyncTlsStream {
    pub async fn connect(host: &str, port: u16) -> crate::Result<Self> {
        let tcp = AsyncTcpStream::connect(host, port).await?;
        let mut record = RecordLayer::new();
        let mut hs = Handshake::new(host)?;

        // Same handshake flow as sync, but with async TCP read/write
        // tcp.write_all() → loop { wait_writable + write }
        // tcp.read() → wait_readable + read

        // ... (mirror the sync implementation with async I/O)

        todo!("implement async handshake — same logic as sync, with await on TCP I/O")
    }

    pub fn raw_fd(&self) -> std::os::unix::io::RawFd {
        self.tcp.raw_fd()
    }

    pub async fn read(&mut self, buf: &mut [u8]) -> crate::Result<usize> {
        // Same as sync Read impl but with async TCP
        todo!()
    }

    pub async fn write_all(&mut self, buf: &[u8]) -> crate::Result<()> {
        // Same as sync Write impl but with async TCP
        todo!()
    }
}
```

Note: The async version is structurally identical to sync. The implementer should extract the handshake logic into shared helper functions that take a generic I/O parameter, or simply duplicate the ~50 lines of handshake flow with `await` calls.

- [ ] **Step 3: Verify compilation**

Run: `cargo build 2>&1 | tail -10`
Expected: Compiles (async has `todo!()` but won't be called yet)

- [ ] **Step 4: Commit**

```bash
git add src/core/net/tls/mod.rs
git commit -m "feat(tls): implement TlsStream + AsyncTlsStream integration layer"
```

---

## Task 12: Migration — Replace Old OpenSSL Code

**Files:**
- Delete: `src/core/net/tls.rs` (already replaced by `tls/` directory)
- Modify: `src/core/net/async_tls.rs` — keep `wait_readable`/`wait_writable` helpers, remove OpenSSL code
- Modify: `tests/core/net/mod.rs`
- Delete: `tests/core/net/tls_test.rs`

- [ ] **Step 1: Update async_tls.rs — keep only the reactor helpers**

The `wait_readable` and `wait_writable` functions in `async_tls.rs` are used by the new `AsyncTlsStream` and should be preserved. Remove only the `AsyncTlsStream` struct and its OpenSSL implementation.

Alternatively, move `wait_readable`/`wait_writable` into a separate utility module (e.g., `src/core/net/async_io.rs`) and delete `async_tls.rs` entirely. Then update imports in the new TLS module.

- [ ] **Step 2: Update module references**

Ensure `src/core/net/mod.rs` no longer exports old `async_tls` OpenSSL code. The new `tls/mod.rs` should be the sole source of `TlsStream` and `AsyncTlsStream`.

Check all imports in:
- `src/llm.rs` — uses `use crate::core::net::tls::TlsStream` and `crate::core::net::async_tls::AsyncTlsStream`
- `src/tools/web.rs` — uses `crate::core::net::async_tls::AsyncTlsStream`

Update import paths to point to new module:
```rust
// Before:
use crate::core::net::tls::TlsStream;
use crate::core::net::async_tls::AsyncTlsStream;

// After:
use crate::core::net::tls::TlsStream;
use crate::core::net::tls::AsyncTlsStream;
```

- [ ] **Step 3: Delete old test file**

Remove `tests/core/net/tls_test.rs`. The new test structure under `tests/core/net/tls/` replaces it.

- [ ] **Step 4: Remove OpenSSL link flags**

Check if there are any build flags or link directives for OpenSSL. Search for `#[link]`, `-lssl`, `-lcrypto` in the codebase. Remove them.

Run: `cargo build 2>&1 | tail -20`
Expected: Builds without linking OpenSSL.

Verify: `ldd target/debug/viv | grep -i ssl`
Expected: No OpenSSL references.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(tls): remove OpenSSL FFI, complete migration to pure Rust TLS 1.3"
```

---

## Task 13: Integration Test — Real HTTPS Connection

**Files:**
- Modify: `tests/core/net/tls/mod.rs`

- [ ] **Step 1: Write end-to-end test**

```rust
// add to tests/core/net/tls/mod.rs (or a new handshake_test.rs)

/// Test real HTTPS GET using the new pure Rust TLS 1.3
#[cfg(feature = "full_test")]
#[test]
fn tls13_https_get_real_server() {
    use viv::core::net::tls::TlsStream;
    use viv::core::net::http::HttpRequest;
    use std::io::{Read, Write};

    let mut tls = TlsStream::connect("www.baidu.com", 443)
        .expect("TLS 1.3 connect failed");

    let req = HttpRequest {
        method: "GET".into(),
        path: "/".into(),
        headers: vec![
            ("Host".into(), "www.baidu.com".into()),
            ("Connection".into(), "close".into()),
        ],
        body: None,
    };

    tls.write_all(&req.to_bytes()).expect("write failed");

    let mut response = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = tls.read(&mut buf).unwrap_or(0);
        if n == 0 { break; }
        response.extend_from_slice(&buf[..n]);
    }

    let resp_str = String::from_utf8_lossy(&response);
    assert!(
        resp_str.starts_with("HTTP/1.1"),
        "Expected HTTP response, got: {}",
        &resp_str[..50.min(resp_str.len())]
    );
    println!("Pure Rust TLS 1.3 response: {} bytes", response.len());
}
```

- [ ] **Step 2: Run integration test**

Run: `cargo test --features full_test tls13_https_get 2>&1`
Expected: PASS — full TLS 1.3 handshake + HTTP request over pure Rust crypto.

- [ ] **Step 3: Run all existing tests to check for regressions**

Run: `cargo test 2>&1 | tail -20`
Expected: All tests pass. No regressions in LLM, tools, or other modules.

- [ ] **Step 4: Verify no OpenSSL dependency**

Run: `cargo build --release && ldd target/release/viv | grep -i ssl`
Expected: Empty — no OpenSSL in the binary.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "test(tls): add TLS 1.3 integration test, verify zero OpenSSL dependency"
```

---

## Execution Order Summary

```
Task 1:  Scaffolding (directory structure, getrandom)
Task 2:  SHA-256
Task 3:  HMAC-SHA256 + HKDF
Task 4:  AES-128 core
Task 5:  AES-128-GCM (GHASH + GCM)
Task 6:  X25519
Task 7:  Key Schedule
Task 8:  Codec
Task 9:  Record Layer
Task 10: Handshake State Machine
Task 11: TlsStream + AsyncTlsStream
Task 12: Migration (delete OpenSSL)
Task 13: Integration Test
```

Tasks 1-7 are pure crypto/math with no I/O — can be developed and tested in isolation. Tasks 2-6 (crypto primitives) are independent of each other and could be parallelized across subagents.

Tasks 8-11 must be sequential (each depends on the previous). Task 12-13 are the final migration and validation.
