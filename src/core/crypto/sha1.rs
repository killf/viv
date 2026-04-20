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
        Self {
            state: H,
            buf: [0u8; 64],
            buf_len: 0,
            total_len: 0,
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        self.total_len += data.len() as u64;
        let mut offset = 0;

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

        while offset + 64 <= data.len() {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[offset..offset + 64]);
            compress(&mut self.state, &block);
            offset += 64;
        }

        let remaining = data.len() - offset;
        if remaining > 0 {
            self.buf[..remaining].copy_from_slice(&data[offset..]);
            self.buf_len = remaining;
        }
    }

    pub fn finish(mut self) -> [u8; 20] {
        let bit_len = self.total_len * 8;

        self.buf[self.buf_len] = 0x80;
        self.buf_len += 1;

        if self.buf_len > 56 {
            for b in &mut self.buf[self.buf_len..64] {
                *b = 0;
            }
            let block = self.buf;
            compress(&mut self.state, &block);
            self.buf_len = 0;
        }

        for b in &mut self.buf[self.buf_len..56] {
            *b = 0;
        }

        self.buf[56..64].copy_from_slice(&bit_len.to_be_bytes());

        let block = self.buf;
        compress(&mut self.state, &block);

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

fn compress(state: &mut [u32; 5], block: &[u8; 64]) {
    let mut w = [0u32; 80];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([
            block[i * 4],
            block[i * 4 + 1],
            block[i * 4 + 2],
            block[i * 4 + 3],
        ]);
    }
    for i in 16..80 {
        w[i] = w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16];
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

// ── HMAC-SHA1 (RFC 2104) ───────────────────────────────────────────

pub fn hmac_sha1(key: &[u8], data: &[u8]) -> [u8; 20] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        let hashed = Sha1::hash(key);
        k[..20].copy_from_slice(&hashed);
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    let mut ipad = [0u8; 64];
    for i in 0..64 {
        ipad[i] = k[i] ^ 0x36;
    }
    let mut inner = Sha1::new();
    inner.update(&ipad);
    inner.update(data);
    let inner_hash = inner.finish();

    let mut opad = [0u8; 64];
    for i in 0..64 {
        opad[i] = k[i] ^ 0x5c;
    }
    let mut outer = Sha1::new();
    outer.update(&opad);
    outer.update(&inner_hash);
    outer.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    // FIPS 180-4 test vectors
    #[test]
    fn test_sha1_empty() {
        assert_eq!(
            Sha1::hash(b""),
            [
                0xda, 0x39, 0xa3, 0xee, 0x5e, 0x6b, 0x4b, 0x0d, 0x32, 0x55, 0xbf, 0xef, 0x95,
                0x60, 0x18, 0x90, 0xaf, 0xd8, 0x07, 0x09,
            ]
        );
    }

    #[test]
    fn test_sha1_abc() {
        assert_eq!(
            Sha1::hash(b"abc"),
            [
                0xa9, 0x99, 0x3e, 0x36, 0x47, 0x06, 0x81, 0x6a, 0xba, 0x3e, 0x25, 0x71, 0x78,
                0x50, 0xc2, 0x6c, 0x9c, 0xd0, 0xd8, 0x9d,
            ]
        );
    }

    #[test]
    fn test_sha1_64_zeros() {
        let input = [0u8; 64];
        assert_eq!(
            Sha1::hash(&input),
            [
                0x0c, 0xd9, 0x1d, 0x06, 0x67, 0x3e, 0xec, 0xc9, 0x4a, 0x14, 0x93, 0xf2, 0x10,
                0x0f, 0xdd, 0x1f, 0xd1, 0x85, 0x85, 0x27,
            ]
        );
    }

    #[test]
    fn test_sha1_multiblock() {
        let data = [0u8; 100];
        let mut inc = Sha1::new();
        inc.update(&data[..50]);
        inc.update(&data[50..]);
        assert_eq!(inc.finish(), Sha1::hash(&data));
    }

    // RFC 2202 test vectors for HMAC-SHA1
    #[test]
    fn test_hmac_sha1_empty() {
        let key = b"";
        let data = b"";
        let expected = [
            0xfb, 0xdb, 0x1d, 0x1b, 0x18, 0xaa, 0x6c, 0x5c, 0x6f, 0xd6, 0x22, 0x2f, 0x42, 0x81,
            0xc8, 0xce, 0x16, 0xc6, 0x89, 0x98,
        ];
        assert_eq!(hmac_sha1(key, data), expected);
    }

    #[test]
    fn test_hmac_sha1_short_key() {
        let key = b"key";
        let data = b"The quick brown fox jumps over the lazy dog";
        let expected = [
            0xde, 0x7c, 0x9b, 0x8b, 0x89, 0x81, 0x17, 0xad, 0x91, 0x5c, 0x2e, 0xc1, 0x93, 0xb0,
            0x47, 0x5a, 0x28, 0x93, 0x61, 0x9b,
        ];
        assert_eq!(hmac_sha1(key, data), expected);
    }
}
