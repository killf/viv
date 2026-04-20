// SHA-1 (FIPS 180-4), HMAC-SHA1 (RFC 2104)
//
// Pure Rust, zero dependencies.

/// Initial hash values H0..H4: first 32 bits of the fractional parts
/// of the square roots of the first 5 primes (2..11).
const H: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];

/// Round constants K0..K79.
const K: [u32; 4] = [
    0x5A827999, // 0..19
    0x6ED9EBA1, // 20..39
    0x8F1BBCDC, // 40..59
    0xCA62C1D6, // 60..79
];

/// SHA-1 incremental hasher.
///
/// Implements FIPS 180-4. Supports incremental feeding via `update`
/// and produces a 20-byte digest via `finish`.
#[derive(Clone)]
pub struct Sha1 {
    state: [u32; 5],
    buf: [u8; 64],
    buf_len: usize,
    total_len: u64,
}

impl Default for Sha1 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha1 {
    pub fn new() -> Self {
        Self { state: H, buf: [0u8; 64], buf_len: 0, total_len: 0 }
    }

    pub fn update(&mut self, data: &[u8]) {
        self.total_len += data.len() as u64;
        let mut offset = 0;

        // Fill partial buffer first
        if self.buf_len > 0 {
            let need = 64 - self.buf_len;
            if data.len() < need {
                self.buf[self.buf_len..self.buf_len + data.len()].copy_from_slice(data);
                self.buf_len += data.len();
                return;
            }
            self.buf[self.buf_len..].copy_from_slice(&data[..need]);
            sha1_compress(&mut self.state, &self.buf);
            self.buf_len = 0;
            offset = need;
        }

        // Process full 64-byte blocks
        while offset + 64 <= data.len() {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[offset..offset + 64]);
            sha1_compress(&mut self.state, &block);
            offset += 64;
        }

        // Remainder
        let remaining = data.len() - offset;
        if remaining > 0 {
            self.buf[..remaining].copy_from_slice(&data[offset..]);
            self.buf_len = remaining;
        }
    }

    pub fn finish(mut self) -> [u8; 20] {
        let bit_len = self.total_len * 8;

        // Append 0x80 padding
        self.buf[self.buf_len] = 0x80;
        self.buf_len += 1;

        // If buf_len > 56, pad remaining with zeros, compress, then restart
        if self.buf_len > 56 {
            self.buf[self.buf_len..].fill(0);
            sha1_compress(&mut self.state, &self.buf);
            self.buf_len = 0;
        }

        // Zero-pad to byte 56, then append 8-byte big-endian bit length
        self.buf[self.buf_len..56].fill(0);
        self.buf[56..64].copy_from_slice(&bit_len.to_be_bytes());
        sha1_compress(&mut self.state, &self.buf);

        // Serialize state as big-endian bytes
        let mut out = [0u8; 20];
        for (i, word) in self.state.iter().enumerate() {
            out[i * 4..(i + 1) * 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }

    pub fn hash(data: &[u8]) -> [u8; 20] {
        let mut h = Self::new();
        h.update(data);
        h.finish()
    }
}

/// SHA-1 compression function for one 64-byte block.
fn sha1_compress(state: &mut [u32; 5], block: &[u8; 64]) {
    debug_assert_eq!(block.len(), 64);

    // W[0..15] = first 16 words (big-endian)
    let mut w = [0u32; 80];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([block[i * 4], block[i * 4 + 1], block[i * 4 + 2], block[i * 4 + 3]]);
    }

    // W[16..79] = ROTL^1(W[i-3] ⊕ W[i-8] ⊕ W[i-14] ⊕ W[i-16])  (FIPS 180-4 §6.1.3)
    for i in 16..80 {
        w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
    }

    let [mut a, mut b, mut c, mut d, mut e] = *state;

    for i in 0..80 {
        let (f, k) = match i {
            0..=19 => ((b & c) | ((!b) & d), K[0]),
            20..=39 => (b ^ c ^ d, K[1]),
            40..=59 => ((b & c) | (b & d) | (c & d), K[2]),
            _ => (b ^ c ^ d, K[3]),
        };
        let temp = a
            .rotate_left(5)
            .wrapping_add(f)
            .wrapping_add(e)
            .wrapping_add(k)
            .wrapping_add(w[i]);
        e = d;
        d = c;
        c = b.rotate_left(30);
        b = a;
        a = temp;
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
}

// ── HMAC-SHA1 (RFC 2104) ─────────────────────────────────────────

pub fn hmac_sha1(key: &[u8], data: &[u8]) -> [u8; 20] {
    // Step 1: normalize key to 64 bytes
    let mut k = [0u8; 64];
    if key.len() > 64 {
        // Keys longer than 64 bytes are hashed first
        k[..20].copy_from_slice(&Sha1::hash(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    // Step 2: XOR key with ipad (0x36) and inner hash
    let mut inner = [0u8; 64];
    for i in 0..64 {
        inner[i] = k[i] ^ 0x36;
    }
    let mut h = Sha1::new();
    h.update(&inner);
    h.update(data);
    let inner_hash = h.finish();

    // Step 3: XOR original key with opad (0x5c) and outer hash
    let mut outer = [0u8; 64];
    for i in 0..64 {
        outer[i] = k[i] ^ 0x5c;
    }
    let mut h = Sha1::new();
    h.update(&outer);
    h.update(&inner_hash);
    h.finish()
}
