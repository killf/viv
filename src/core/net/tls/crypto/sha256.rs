// SHA-256 (FIPS 180-4), HMAC-SHA256 (RFC 2104), HKDF (RFC 5869)
//
// Pure Rust, zero dependencies. These are the foundational crypto
// primitives for the TLS 1.3 handshake and record layer.

// ── SHA-256 ──────────────────────────────────────────────────────────

/// Initial hash values H0..H7: first 32 bits of the fractional parts
/// of the square roots of the first 8 primes (2..19).
const H: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// Round constants K0..K63: first 32 bits of the fractional parts
/// of the cube roots of the first 64 primes (2..311).
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// SHA-256 incremental hasher.
///
/// Implements FIPS 180-4. Supports incremental feeding via `update`
/// and produces a 32-byte digest via `finish`. `Clone` is required
/// for transcript hash forking during TLS handshakes.
#[derive(Clone)]
pub struct Sha256 {
    state: [u32; 8],
    buf: [u8; 64],
    buf_len: usize,
    total_len: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    /// Create a new hasher initialised with H0..H7.
    pub fn new() -> Self {
        Self {
            state: H,
            buf: [0u8; 64],
            buf_len: 0,
            total_len: 0,
        }
    }

    /// Feed data into the hasher. Can be called multiple times.
    pub fn update(&mut self, data: &[u8]) {
        self.total_len += data.len() as u64;
        let mut offset = 0;

        // If there's buffered data, try to complete a block
        if self.buf_len > 0 {
            let need = 64 - self.buf_len;
            if data.len() < need {
                self.buf[self.buf_len..self.buf_len + data.len()].copy_from_slice(data);
                self.buf_len += data.len();
                return;
            }
            self.buf[self.buf_len..64].copy_from_slice(&data[..need]);
            let block = self.buf;
            compress(&mut self.state, &block);
            self.buf_len = 0;
            offset = need;
        }

        // Process full 64-byte blocks directly from data
        while offset + 64 <= data.len() {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[offset..offset + 64]);
            compress(&mut self.state, &block);
            offset += 64;
        }

        // Buffer remaining bytes
        let remaining = data.len() - offset;
        if remaining > 0 {
            self.buf[..remaining].copy_from_slice(&data[offset..]);
            self.buf_len = remaining;
        }
    }

    /// Finalize the hash and return the 32-byte digest.
    /// Consumes the hasher (use `clone()` to fork first).
    pub fn finish(mut self) -> [u8; 32] {
        let bit_len = self.total_len * 8;

        // Append the 0x80 byte
        self.buf[self.buf_len] = 0x80;
        self.buf_len += 1;

        // If not enough room for the 8-byte length, pad and compress
        if self.buf_len > 56 {
            // Zero-fill rest of block
            for b in &mut self.buf[self.buf_len..64] {
                *b = 0;
            }
            let block = self.buf;
            compress(&mut self.state, &block);
            self.buf_len = 0;
        }

        // Zero-fill up to byte 56
        for b in &mut self.buf[self.buf_len..56] {
            *b = 0;
        }

        // Append 64-bit big-endian bit length
        self.buf[56..64].copy_from_slice(&bit_len.to_be_bytes());

        let block = self.buf;
        compress(&mut self.state, &block);

        // Produce the final 32-byte hash
        let mut out = [0u8; 32];
        for (i, word) in self.state.iter().enumerate() {
            out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }

    /// One-shot convenience: hash `data` and return the 32-byte digest.
    pub fn hash(data: &[u8]) -> [u8; 32] {
        let mut h = Self::new();
        h.update(data);
        h.finish()
    }
}

/// SHA-256 compression function. Processes one 512-bit (64-byte) block.
fn compress(state: &mut [u32; 8], block: &[u8; 64]) {
    // Prepare message schedule W[0..63]
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

    // Working variables
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;

    // 64 rounds
    for i in 0..64 {
        let big_s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let temp1 = h
            .wrapping_add(big_s1)
            .wrapping_add(ch)
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        let big_s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = big_s0.wrapping_add(maj);

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp1);
        d = c;
        c = b;
        b = a;
        a = temp1.wrapping_add(temp2);
    }

    // Add compressed chunk to current hash value
    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

// ── HMAC-SHA256 (RFC 2104) ───────────────────────────────────────────

/// Compute HMAC-SHA256(key, data).
///
/// RFC 2104: If key > 64 bytes, hash it first. XOR with ipad (0x36)
/// for inner hash, opad (0x5c) for outer hash.
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    // Step 1: normalise key to exactly 64 bytes
    let mut k = [0u8; 64];
    if key.len() > 64 {
        let hashed = Sha256::hash(key);
        k[..32].copy_from_slice(&hashed);
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    // Step 2: inner hash = SHA256((K ^ ipad) || data)
    let mut ipad = [0u8; 64];
    for i in 0..64 {
        ipad[i] = k[i] ^ 0x36;
    }
    let mut inner = Sha256::new();
    inner.update(&ipad);
    inner.update(data);
    let inner_hash = inner.finish();

    // Step 3: outer hash = SHA256((K ^ opad) || inner_hash)
    let mut opad = [0u8; 64];
    for i in 0..64 {
        opad[i] = k[i] ^ 0x5c;
    }
    let mut outer = Sha256::new();
    outer.update(&opad);
    outer.update(&inner_hash);
    outer.finish()
}

// ── HKDF (RFC 5869) ─────────────────────────────────────────────────

/// HKDF-Extract: derive a pseudorandom key from input keying material.
///
/// PRK = HMAC-SHA256(salt, IKM). If salt is empty, use 32 zero bytes.
pub fn hkdf_extract(salt: &[u8], ikm: &[u8]) -> [u8; 32] {
    if salt.is_empty() {
        hmac_sha256(&[0u8; 32], ikm)
    } else {
        hmac_sha256(salt, ikm)
    }
}

/// HKDF-Expand: expand a pseudorandom key to the desired length.
///
/// T(0) = empty, T(i) = HMAC-SHA256(PRK, T(i-1) || info || i).
/// Output is the first `out.len()` bytes of T(1) || T(2) || ...
///
/// Panics if `out.len()` > 255 * 32 (per RFC 5869).
pub fn hkdf_expand(prk: &[u8], info: &[u8], out: &mut [u8]) {
    let n = out.len().div_ceil(32); // ceil(L / HashLen)
    assert!(n <= 255, "HKDF-Expand: output too long");

    let t_prev = [0u8; 0]; // T(0) is empty
    let mut offset = 0;

    for i in 1..=n {
        let mut hmac_input = Vec::new();
        if i > 1 {
            // Append T(i-1) — stored in out[offset-32..offset]
            hmac_input.extend_from_slice(&out[offset - 32..offset]);
        } else {
            hmac_input.extend_from_slice(&t_prev);
        }
        hmac_input.extend_from_slice(info);
        hmac_input.push(i as u8);

        let t = hmac_sha256(prk, &hmac_input);

        let remaining = out.len() - offset;
        let to_copy = if remaining < 32 { remaining } else { 32 };
        out[offset..offset + to_copy].copy_from_slice(&t[..to_copy]);
        offset += to_copy;
    }
}
