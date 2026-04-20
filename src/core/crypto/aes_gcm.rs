// AES-128 (FIPS 197) + AES-128-GCM (NIST SP 800-38D)
//
// Pure Rust, zero dependencies. AES-128-GCM is the AEAD cipher used
// by the TLS_AES_128_GCM_SHA256 cipher suite in TLS 1.3.

// ── AES-128 constants ───────────────────────────────────────────────

/// AES S-Box (SubBytes lookup table)
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

/// Round constants for AES-128 key expansion (indices 1..10 used)
const RCON: [u8; 11] = [
    0x00, 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36,
];

// ── AES-128 ─────────────────────────────────────────────────────────

/// AES-128 block cipher (encrypt only).
///
/// AES-128 uses a 128-bit key expanded into 11 round keys (initial + 10
/// rounds). The state is a 4x4 column-major byte matrix.
pub struct Aes128 {
    round_keys: [[u8; 16]; 11],
}

impl Aes128 {
    /// Create a new AES-128 cipher from a 16-byte key.
    pub fn new(key: &[u8; 16]) -> Self {
        let mut round_keys = [[0u8; 16]; 11];
        round_keys[0] = *key;

        for i in 1..11 {
            let prev = round_keys[i - 1];
            let mut rk = [0u8; 16];

            // First word: RotWord + SubWord + RCON on last word of previous key
            let w3 = [prev[12], prev[13], prev[14], prev[15]];
            // RotWord: [a,b,c,d] → [b,c,d,a]
            let rot = [w3[1], w3[2], w3[3], w3[0]];
            // SubWord: S-Box each byte
            let sub = [
                SBOX[rot[0] as usize],
                SBOX[rot[1] as usize],
                SBOX[rot[2] as usize],
                SBOX[rot[3] as usize],
            ];

            rk[0] = prev[0] ^ sub[0] ^ RCON[i];
            rk[1] = prev[1] ^ sub[1];
            rk[2] = prev[2] ^ sub[2];
            rk[3] = prev[3] ^ sub[3];

            // Remaining three words: XOR chain
            for w in 1..4 {
                let base = w * 4;
                rk[base] = rk[base - 4] ^ prev[base];
                rk[base + 1] = rk[base - 3] ^ prev[base + 1];
                rk[base + 2] = rk[base - 2] ^ prev[base + 2];
                rk[base + 3] = rk[base - 1] ^ prev[base + 3];
            }

            round_keys[i] = rk;
        }

        Aes128 { round_keys }
    }

    /// Encrypt a single 16-byte block.
    pub fn encrypt_block(&self, block: &[u8; 16]) -> [u8; 16] {
        let mut state = *block;

        // Initial round key addition
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

/// XOR a 16-byte block in place.
fn xor_block(a: &mut [u8; 16], b: &[u8; 16]) {
    for i in 0..16 {
        a[i] ^= b[i];
    }
}

/// SubBytes: replace each byte with its S-Box value.
fn sub_bytes(state: &mut [u8; 16]) {
    for b in state.iter_mut() {
        *b = SBOX[*b as usize];
    }
}

/// ShiftRows on column-major state.
///
/// AES state layout (column-major, each column = 4 consecutive bytes):
///   state[0]  state[4]  state[8]  state[12]   ← row 0 (no shift)
///   state[1]  state[5]  state[9]  state[13]   ← row 1 (shift left 1)
///   state[2]  state[6]  state[10] state[14]   ← row 2 (shift left 2)
///   state[3]  state[7]  state[11] state[15]   ← row 3 (shift left 3)
fn shift_rows(state: &mut [u8; 16]) {
    // Row 1: shift left by 1
    let t = state[1];
    state[1] = state[5];
    state[5] = state[9];
    state[9] = state[13];
    state[13] = t;

    // Row 2: shift left by 2
    let t0 = state[2];
    let t1 = state[6];
    state[2] = state[10];
    state[6] = state[14];
    state[10] = t0;
    state[14] = t1;

    // Row 3: shift left by 3 (= right by 1)
    let t = state[15];
    state[15] = state[11];
    state[11] = state[7];
    state[7] = state[3];
    state[3] = t;
}

/// xtime: multiply by x in GF(2^8) with reduction polynomial x^8 + x^4 + x^3 + x + 1.
fn xtime(a: u8) -> u8 {
    let shifted = (a as u16) << 1;
    let reduced = shifted ^ (if a & 0x80 != 0 { 0x1b } else { 0x00 });
    reduced as u8
}

/// MixColumns: each column is treated as a polynomial over GF(2^8).
fn mix_columns(state: &mut [u8; 16]) {
    for col in 0..4 {
        let i = col * 4;
        let a0 = state[i];
        let a1 = state[i + 1];
        let a2 = state[i + 2];
        let a3 = state[i + 3];
        let t = a0 ^ a1 ^ a2 ^ a3;
        state[i] = a0 ^ xtime(a0 ^ a1) ^ t;
        state[i + 1] = a1 ^ xtime(a1 ^ a2) ^ t;
        state[i + 2] = a2 ^ xtime(a2 ^ a3) ^ t;
        state[i + 3] = a3 ^ xtime(a3 ^ a0) ^ t;
    }
}

// ── GHASH (GF(2^128) multiplication) ────────────────────────────────

/// Multiply two 128-bit blocks in GF(2^128) with reduction polynomial
/// x^128 + x^7 + x^2 + x + 1 (bit-by-bit algorithm).
fn ghash_mult(x: &[u8; 16], y: &[u8; 16]) -> [u8; 16] {
    let mut z = [0u8; 16];
    let mut v = *y;

    for i in 0..128 {
        // If bit i of x is set (MSB-first ordering)
        if x[i / 8] & (0x80 >> (i % 8)) != 0 {
            for j in 0..16 {
                z[j] ^= v[j];
            }
        }

        // Check if the LSB of V is set (for reduction)
        let lsb = v[15] & 1;

        // Right-shift V by 1 bit
        let mut carry = 0u8;
        for byte in &mut v {
            let new_carry = *byte & 1;
            *byte = (*byte >> 1) | (carry << 7);
            carry = new_carry;
        }

        // If LSB was set, XOR with reduction constant R = 0xe1000000...
        if lsb != 0 {
            v[0] ^= 0xe1;
        }
    }

    z
}

/// GHASH: compute GHASH(H, data) = successive multiply-accumulate in GF(2^128).
fn ghash(h: &[u8; 16], data: &[u8]) -> [u8; 16] {
    let mut y = [0u8; 16];
    let mut i = 0;
    while i + 16 <= data.len() {
        for j in 0..16 {
            y[j] ^= data[i + j];
        }
        y = ghash_mult(&y, h);
        i += 16;
    }
    // Handle any remaining partial block (shouldn't happen when called
    // with properly padded data, but be safe)
    if i < data.len() {
        let mut block = [0u8; 16];
        block[..data.len() - i].copy_from_slice(&data[i..]);
        for j in 0..16 {
            y[j] ^= block[j];
        }
        y = ghash_mult(&y, h);
    }
    y
}

// ── AES-128-GCM ─────────────────────────────────────────────────────

/// AES-128-GCM authenticated encryption with associated data (AEAD).
///
/// Implements NIST SP 800-38D using AES-128 as the block cipher.
pub struct Aes128Gcm {
    aes: Aes128,
    /// GHASH subkey: H = AES(key, 0^128)
    h: [u8; 16],
}

impl Aes128Gcm {
    /// Create a new AES-128-GCM cipher from a 16-byte key.
    pub fn new(key: &[u8; 16]) -> Self {
        let aes = Aes128::new(key);
        let h = aes.encrypt_block(&[0u8; 16]);
        Aes128Gcm { aes, h }
    }

    /// Encrypt plaintext with AES-128-GCM.
    ///
    /// `out` must be at least `plaintext.len() + 16` bytes (ciphertext + tag).
    pub fn encrypt(&self, nonce: &[u8; 12], aad: &[u8], plaintext: &[u8], out: &mut [u8]) -> crate::Result<()> {
        if out.len() < plaintext.len() + 16 {
            return Err(crate::Error::Invariant(
                "GCM encrypt: output buffer too small".into(),
            ));
        }

        // J0 = nonce || 0x00000001
        let mut j0 = [0u8; 16];
        j0[..12].copy_from_slice(nonce);
        j0[15] = 1;

        // Encrypt J0 for final tag XOR
        let enc_j0 = self.aes.encrypt_block(&j0);

        // CTR encryption starting from inc32(J0)
        let mut counter = j0;
        let ct_len = plaintext.len();
        let mut offset = 0;

        while offset < ct_len {
            inc32(&mut counter);
            let keystream = self.aes.encrypt_block(&counter);
            let chunk_len = core::cmp::min(16, ct_len - offset);
            for i in 0..chunk_len {
                out[offset + i] = plaintext[offset + i] ^ keystream[i];
            }
            offset += chunk_len;
        }

        // Build GHASH input: pad(AAD) || pad(CT) || len_block
        let tag = self.compute_tag(aad, &out[..ct_len], &enc_j0);
        out[ct_len..ct_len + 16].copy_from_slice(&tag);
        Ok(())
    }

    /// Decrypt ciphertext with AES-128-GCM.
    ///
    /// `ciphertext_and_tag` = ciphertext || 16-byte authentication tag.
    /// `out` must be at least `ciphertext_and_tag.len() - 16` bytes.
    /// Returns the plaintext length, or `Err` if authentication fails.
    pub fn decrypt(
        &self,
        nonce: &[u8; 12],
        aad: &[u8],
        ciphertext_and_tag: &[u8],
        out: &mut [u8],
    ) -> crate::Result<usize> {
        if ciphertext_and_tag.len() < 16 {
            return Err(crate::Error::Tls("GCM: ciphertext too short".into()));
        }

        let ct_len = ciphertext_and_tag.len() - 16;
        let ct = &ciphertext_and_tag[..ct_len];
        let received_tag = &ciphertext_and_tag[ct_len..];

        // J0 = nonce || 0x00000001
        let mut j0 = [0u8; 16];
        j0[..12].copy_from_slice(nonce);
        j0[15] = 1;

        let enc_j0 = self.aes.encrypt_block(&j0);

        // Compute expected tag BEFORE decryption (authenticate-then-decrypt)
        let expected_tag = self.compute_tag(aad, ct, &enc_j0);

        // Constant-time tag comparison
        let mut diff = 0u8;
        for i in 0..16 {
            diff |= received_tag[i] ^ expected_tag[i];
        }
        if diff != 0 {
            return Err(crate::Error::Tls("GCM: authentication failed".into()));
        }

        // CTR decryption (same as encryption)
        let mut counter = j0;
        let mut offset = 0;

        while offset < ct_len {
            inc32(&mut counter);
            let keystream = self.aes.encrypt_block(&counter);
            let chunk_len = core::cmp::min(16, ct_len - offset);
            for i in 0..chunk_len {
                out[offset + i] = ct[offset + i] ^ keystream[i];
            }
            offset += chunk_len;
        }

        Ok(ct_len)
    }

    /// Compute the GCM authentication tag.
    ///
    /// tag = GHASH(H, pad(AAD) || pad(CT) || len_block) XOR AES(K, J0)
    fn compute_tag(&self, aad: &[u8], ct: &[u8], enc_j0: &[u8; 16]) -> [u8; 16] {
        // Build GHASH input: pad(AAD) || pad(CT) || len_block
        let aad_padded = pad16_len(aad.len());
        let ct_padded = pad16_len(ct.len());
        let ghash_input_len = aad_padded + ct_padded + 16;

        let mut ghash_input = vec![0u8; ghash_input_len];
        ghash_input[..aad.len()].copy_from_slice(aad);
        // Zero padding for AAD is already there (vec initialized to 0)
        ghash_input[aad_padded..aad_padded + ct.len()].copy_from_slice(ct);
        // Zero padding for CT is already there

        // Length block: [len(AAD) in bits as u64 BE] || [len(CT) in bits as u64 BE]
        let aad_bits = (aad.len() as u64) * 8;
        let ct_bits = (ct.len() as u64) * 8;
        let len_offset = aad_padded + ct_padded;
        ghash_input[len_offset..len_offset + 8].copy_from_slice(&aad_bits.to_be_bytes());
        ghash_input[len_offset + 8..len_offset + 16].copy_from_slice(&ct_bits.to_be_bytes());

        let mut tag = ghash(&self.h, &ghash_input);

        // XOR with encrypted J0
        for i in 0..16 {
            tag[i] ^= enc_j0[i];
        }

        tag
    }
}

/// Increment the rightmost 32 bits of a 16-byte counter (big-endian).
fn inc32(counter: &mut [u8; 16]) {
    let mut c = u32::from_be_bytes([counter[12], counter[13], counter[14], counter[15]]);
    c = c.wrapping_add(1);
    counter[12..16].copy_from_slice(&c.to_be_bytes());
}

/// Round up to the next multiple of 16.
fn pad16_len(len: usize) -> usize {
    if len == 0 { 0 } else { (len + 15) & !15 }
}
