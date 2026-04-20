// X25519 Elliptic Curve Diffie-Hellman key exchange (RFC 7748)
//
// Curve25519 over GF(2^255 - 19) using Montgomery ladder.
// Field elements are represented as 5 limbs of 51 bits each (u64).

/// Field element: 5 × 51-bit limbs in radix 2^51.
/// Limb i holds bits [i*51 .. (i+1)*51).
type Fe = [u64; 5];

const MASK51: u64 = (1u64 << 51) - 1;

// ── Field element serialization ─────────────────────────────────────

/// Decode 32 little-endian bytes into a field element, clearing the top bit.
fn fe_frombytes(s: &[u8; 32]) -> Fe {
    // Read as a 256-bit little-endian integer, split into 51-bit limbs.
    // We clear bit 255 (top bit of byte 31) as required by RFC 7748.
    let mut h = [0u64; 5];
    // Load bytes as u64 chunks, then extract 51-bit limbs.
    let load8 = |b: &[u8]| -> u64 {
        let mut v = 0u64;
        for (i, &byte) in b.iter().take(8).enumerate() {
            v |= (byte as u64) << (8 * i);
        }
        v
    };

    let mut raw = [0u8; 32];
    raw.copy_from_slice(s);
    raw[31] &= 127; // clear top bit

    // Extract 51-bit limbs from 256-bit little-endian integer.
    // Limb 0: bits [0..51)
    // Limb 1: bits [51..102)
    // Limb 2: bits [102..153)
    // Limb 3: bits [153..204)
    // Limb 4: bits [204..255)
    h[0] = load8(&raw[0..]) & MASK51;
    h[1] = (load8(&raw[6..]) >> 3) & MASK51;
    h[2] = (load8(&raw[12..]) >> 6) & MASK51;
    h[3] = (load8(&raw[19..]) >> 1) & MASK51;
    h[4] = (load8(&raw[24..]) >> 12) & MASK51;

    h
}

/// Encode a field element to 32 little-endian bytes after full reduction.
fn fe_tobytes(h: &Fe) -> [u8; 32] {
    let mut t = *h;
    fe_reduce(&mut t);

    // Combine 51-bit limbs into a 256-bit little-endian integer.
    // Use u128 accumulator to handle cross-limb boundaries cleanly.
    let mut s = [0u8; 32];
    let mut acc: u128 = 0;
    let mut acc_bits: u32 = 0;
    let mut byte_pos: usize = 0;

    for &limb in t.iter() {
        acc |= (limb as u128) << acc_bits;
        acc_bits += 51;

        while acc_bits >= 8 && byte_pos < 32 {
            s[byte_pos] = acc as u8;
            acc >>= 8;
            acc_bits -= 8;
            byte_pos += 1;
        }
    }
    // Flush any remaining bits
    if byte_pos < 32 {
        s[byte_pos] = acc as u8;
    }

    s
}

// ── Field arithmetic ────────────────────────────────────────────────

fn fe_add(a: &Fe, b: &Fe) -> Fe {
    [
        a[0].wrapping_add(b[0]),
        a[1].wrapping_add(b[1]),
        a[2].wrapping_add(b[2]),
        a[3].wrapping_add(b[3]),
        a[4].wrapping_add(b[4]),
    ]
}

/// Subtraction: a - b, adding a bias to avoid underflow.
///
/// The bias is 2*(2^51-19) for limb 0 and 2*(2^51-1) for limbs 1-4,
/// which is congruent to 0 mod p and large enough to prevent underflow.
fn fe_sub(a: &Fe, b: &Fe) -> Fe {
    const BIAS0: u64 = 0xFFFFFFFFFFFDA; // 2 * (2^51 - 19)
    const BIAS: u64 = 0xFFFFFFFFFFFFE; // 2 * (2^51 - 1)

    [
        (a[0].wrapping_add(BIAS0)).wrapping_sub(b[0]),
        (a[1].wrapping_add(BIAS)).wrapping_sub(b[1]),
        (a[2].wrapping_add(BIAS)).wrapping_sub(b[2]),
        (a[3].wrapping_add(BIAS)).wrapping_sub(b[3]),
        (a[4].wrapping_add(BIAS)).wrapping_sub(b[4]),
    ]
}

/// Schoolbook multiplication with u128 intermediates, reduced mod p.
fn fe_mul(a: &Fe, b: &Fe) -> Fe {
    let a0 = a[0] as u128;
    let a1 = a[1] as u128;
    let a2 = a[2] as u128;
    let a3 = a[3] as u128;
    let a4 = a[4] as u128;

    let b0 = b[0] as u128;
    let b1 = b[1] as u128;
    let b2 = b[2] as u128;
    let b3 = b[3] as u128;
    let b4 = b[4] as u128;

    // Precompute 19*b_i for reduction: 2^255 ≡ 19 (mod p)
    // When a_i * b_j lands in limb >= 5, we fold it back with factor 19.
    let b1_19 = 19 * b1;
    let b2_19 = 19 * b2;
    let b3_19 = 19 * b3;
    let b4_19 = 19 * b4;

    // Accumulate into 5 limbs:
    // r0 = a0*b0 + 19*(a1*b4 + a2*b3 + a3*b2 + a4*b1)
    // r1 = a0*b1 + a1*b0 + 19*(a2*b4 + a3*b3 + a4*b2)
    // r2 = a0*b2 + a1*b1 + a2*b0 + 19*(a3*b4 + a4*b3)
    // r3 = a0*b3 + a1*b2 + a2*b1 + a3*b0 + 19*(a4*b4)
    // r4 = a0*b4 + a1*b3 + a2*b2 + a3*b1 + a4*b0
    let mut r0 = a0 * b0 + a1 * b4_19 + a2 * b3_19 + a3 * b2_19 + a4 * b1_19;
    let mut r1 = a0 * b1 + a1 * b0 + a2 * b4_19 + a3 * b3_19 + a4 * b2_19;
    let mut r2 = a0 * b2 + a1 * b1 + a2 * b0 + a3 * b4_19 + a4 * b3_19;
    let mut r3 = a0 * b3 + a1 * b2 + a2 * b1 + a3 * b0 + a4 * b4_19;
    let mut r4 = a0 * b4 + a1 * b3 + a2 * b2 + a3 * b1 + a4 * b0;

    // Carry propagation
    let mask51 = MASK51 as u128;

    let c = r0 >> 51;
    r0 &= mask51;
    r1 += c;
    let c = r1 >> 51;
    r1 &= mask51;
    r2 += c;
    let c = r2 >> 51;
    r2 &= mask51;
    r3 += c;
    let c = r3 >> 51;
    r3 &= mask51;
    r4 += c;
    let c = r4 >> 51;
    r4 &= mask51;
    r0 += c * 19;
    // One more carry from r0 in case the *19 pushed it over
    let c = r0 >> 51;
    r0 &= mask51;
    r1 += c;

    [r0 as u64, r1 as u64, r2 as u64, r3 as u64, r4 as u64]
}

/// Squaring (optimized: exploit a_i * a_j == a_j * a_i symmetry).
fn fe_sq(a: &Fe) -> Fe {
    let a0 = a[0] as u128;
    let a1 = a[1] as u128;
    let a2 = a[2] as u128;
    let a3 = a[3] as u128;
    let a4 = a[4] as u128;

    // r0 = a0^2 + 19*(2*a1*a4 + 2*a2*a3)
    // r1 = 2*a0*a1 + 19*(2*a2*a4 + a3^2)
    // r2 = 2*a0*a2 + a1^2 + 19*(2*a3*a4)
    // r3 = 2*a0*a3 + 2*a1*a2 + 19*a4^2
    // r4 = 2*a0*a4 + 2*a1*a3 + a2^2
    let mut r0 = a0 * a0 + 38 * a1 * a4 + 38 * a2 * a3;
    let mut r1 = 2 * a0 * a1 + 38 * a2 * a4 + 19 * a3 * a3;
    let mut r2 = 2 * a0 * a2 + a1 * a1 + 38 * a3 * a4;
    let mut r3 = 2 * a0 * a3 + 2 * a1 * a2 + 19 * a4 * a4;
    let mut r4 = 2 * a0 * a4 + 2 * a1 * a3 + a2 * a2;

    // Carry propagation
    let mask51 = MASK51 as u128;

    let c = r0 >> 51;
    r0 &= mask51;
    r1 += c;
    let c = r1 >> 51;
    r1 &= mask51;
    r2 += c;
    let c = r2 >> 51;
    r2 &= mask51;
    r3 += c;
    let c = r3 >> 51;
    r3 &= mask51;
    r4 += c;
    let c = r4 >> 51;
    r4 &= mask51;
    r0 += c * 19;
    let c = r0 >> 51;
    r0 &= mask51;
    r1 += c;

    [r0 as u64, r1 as u64, r2 as u64, r3 as u64, r4 as u64]
}

/// Full reduction to [0, p). After this, each limb is strictly < 2^51
/// and the value is in canonical form.
fn fe_reduce(h: &mut Fe) {
    // First, carry to ensure each limb < 2^52 roughly
    let mut c;
    c = h[0] >> 51;
    h[0] &= MASK51;
    h[1] = h[1].wrapping_add(c);
    c = h[1] >> 51;
    h[1] &= MASK51;
    h[2] = h[2].wrapping_add(c);
    c = h[2] >> 51;
    h[2] &= MASK51;
    h[3] = h[3].wrapping_add(c);
    c = h[3] >> 51;
    h[3] &= MASK51;
    h[4] = h[4].wrapping_add(c);
    c = h[4] >> 51;
    h[4] &= MASK51;
    h[0] = h[0].wrapping_add(c.wrapping_mul(19));

    // Second pass
    c = h[0] >> 51;
    h[0] &= MASK51;
    h[1] = h[1].wrapping_add(c);
    c = h[1] >> 51;
    h[1] &= MASK51;
    h[2] = h[2].wrapping_add(c);
    c = h[2] >> 51;
    h[2] &= MASK51;
    h[3] = h[3].wrapping_add(c);
    c = h[3] >> 51;
    h[3] &= MASK51;
    h[4] = h[4].wrapping_add(c);
    c = h[4] >> 51;
    h[4] &= MASK51;
    h[0] = h[0].wrapping_add(c.wrapping_mul(19));

    // Now subtract p if h >= p.
    // p = 2^255 - 19, in limbs: [2^51 - 19, 2^51 - 1, 2^51 - 1, 2^51 - 1, 2^51 - 1]
    // We compute h - p. If the result doesn't borrow (h >= p), use it.
    // Otherwise keep h.

    // Add 19 to limb 0 (equivalent to subtracting p and adding 2^255,
    // then checking if result >= 2^255).
    let mut t = [0u64; 5];
    t[0] = h[0].wrapping_add(19);
    c = t[0] >> 51;
    t[0] &= MASK51;
    t[1] = h[1].wrapping_add(c);
    c = t[1] >> 51;
    t[1] &= MASK51;
    t[2] = h[2].wrapping_add(c);
    c = t[2] >> 51;
    t[2] &= MASK51;
    t[3] = h[3].wrapping_add(c);
    c = t[3] >> 51;
    t[3] &= MASK51;
    t[4] = h[4].wrapping_add(c);

    // If bit 51 of t[4] is set, then h+19 >= 2^255, meaning h >= p.
    // In that case, use t (with bit 51 cleared) as the result.
    let carry = t[4] >> 51;
    t[4] &= MASK51;

    // carry is 1 if h >= p, 0 otherwise.
    // mask = 0 if carry == 0, all-ones if carry == 1.
    let mask = 0u64.wrapping_sub(carry);

    for i in 0..5 {
        h[i] = (h[i] & !mask) | (t[i] & mask);
    }
}

/// Modular inversion: a^(p-2) mod p via addition chain.
/// p - 2 = 2^255 - 21
fn fe_invert(a: &Fe) -> Fe {
    // Standard addition chain for Curve25519 inversion.
    // Based on the well-known chain from donna/ref10.
    //
    // Compute z^(2^255 - 21) = z^(p-2).

    let z1 = *a; // z^1
    let z2 = fe_sq(&z1); // z^2
    let z8 = {
        // z^8
        let t = fe_sq(&z2); // z^4
        fe_sq(&t) // z^8
    };
    let z9 = fe_mul(&z8, &z1); // z^9
    let z11 = fe_mul(&z9, &z2); // z^11
    let z_5_0 = fe_sq(&z11); // z^22
    let z_5_0 = fe_mul(&z_5_0, &z9); // z^(2^5 - 1) = z^31

    // z^(2^10 - 1)
    let mut z_10_0 = fe_sq(&z_5_0); // z^(2^6 - 2)
    for _ in 1..5 {
        z_10_0 = fe_sq(&z_10_0);
    }
    let z_10_0 = fe_mul(&z_10_0, &z_5_0); // z^(2^10 - 1)

    // z^(2^20 - 1)
    let mut z_20_0 = fe_sq(&z_10_0);
    for _ in 1..10 {
        z_20_0 = fe_sq(&z_20_0);
    }
    let z_20_0 = fe_mul(&z_20_0, &z_10_0);

    // z^(2^40 - 1)
    let mut z_40_0 = fe_sq(&z_20_0);
    for _ in 1..20 {
        z_40_0 = fe_sq(&z_40_0);
    }
    let z_40_0 = fe_mul(&z_40_0, &z_20_0);

    // z^(2^50 - 1)
    let mut z_50_0 = fe_sq(&z_40_0);
    for _ in 1..10 {
        z_50_0 = fe_sq(&z_50_0);
    }
    let z_50_0 = fe_mul(&z_50_0, &z_10_0);

    // z^(2^100 - 1)
    let mut z_100_0 = fe_sq(&z_50_0);
    for _ in 1..50 {
        z_100_0 = fe_sq(&z_100_0);
    }
    let z_100_0 = fe_mul(&z_100_0, &z_50_0);

    // z^(2^200 - 1)
    let mut z_200_0 = fe_sq(&z_100_0);
    for _ in 1..100 {
        z_200_0 = fe_sq(&z_200_0);
    }
    let z_200_0 = fe_mul(&z_200_0, &z_100_0);

    // z^(2^250 - 1)
    let mut z_250_0 = fe_sq(&z_200_0);
    for _ in 1..50 {
        z_250_0 = fe_sq(&z_250_0);
    }
    let z_250_0 = fe_mul(&z_250_0, &z_50_0);

    // z^(2^255 - 2^5)
    let mut t = fe_sq(&z_250_0);
    for _ in 1..5 {
        t = fe_sq(&t);
    }

    // z^(2^255 - 21)  =  z^(2^255 - 32 + 11)  =  z^(2^255 - 2^5) * z^11
    fe_mul(&t, &z11)
}

/// Constant-time conditional swap.
fn fe_cswap(a: &mut Fe, b: &mut Fe, swap: u64) {
    let mask = 0u64.wrapping_sub(swap); // 0 or 0xFFFFFFFFFFFFFFFF
    for i in 0..5 {
        let t = mask & (a[i] ^ b[i]);
        a[i] ^= t;
        b[i] ^= t;
    }
}

/// Scalar clamping per RFC 7748 §5.
fn clamp(scalar: &mut [u8; 32]) {
    scalar[0] &= 248; // clear bottom 3 bits
    scalar[31] &= 127; // clear top bit
    scalar[31] |= 64; // set second-to-top bit
}

// ── Public API ──────────────────────────────────────────────────────

/// Low-level scalar multiplication: result = scalar * point.
///
/// Implements the Montgomery ladder from RFC 7748 §5.
/// The scalar is clamped internally per the RFC.
pub fn scalarmult(scalar: &[u8; 32], point: &[u8; 32]) -> [u8; 32] {
    let mut k = *scalar;
    clamp(&mut k);

    let u = fe_frombytes(point);

    // Montgomery ladder state
    let mut x_2: Fe = [1, 0, 0, 0, 0]; // x_2 = 1
    let mut z_2: Fe = [0, 0, 0, 0, 0]; // z_2 = 0
    let mut x_3 = u; // x_3 = u
    let mut z_3: Fe = [1, 0, 0, 0, 0]; // z_3 = 1

    let a24: Fe = [121665, 0, 0, 0, 0]; // (A-2)/4 for Curve25519

    let mut swap: u64 = 0;

    // Process bits from 254 down to 0
    for pos in (0..=254).rev() {
        let byte_idx = pos / 8;
        let bit_idx = pos % 8;
        let k_t = ((k[byte_idx] >> bit_idx) & 1) as u64;

        swap ^= k_t;
        fe_cswap(&mut x_2, &mut x_3, swap);
        fe_cswap(&mut z_2, &mut z_3, swap);
        swap = k_t;

        let a = fe_add(&x_2, &z_2);
        let aa = fe_sq(&a);
        let b = fe_sub(&x_2, &z_2);
        let bb = fe_sq(&b);
        let e = fe_sub(&aa, &bb);
        let c = fe_add(&x_3, &z_3);
        let d = fe_sub(&x_3, &z_3);
        let da = fe_mul(&d, &a);
        let cb = fe_mul(&c, &b);

        let da_cb_sum = fe_add(&da, &cb);
        x_3 = fe_sq(&da_cb_sum);

        let da_cb_diff = fe_sub(&da, &cb);
        let da_cb_diff_sq = fe_sq(&da_cb_diff);
        z_3 = fe_mul(&u, &da_cb_diff_sq);

        x_2 = fe_mul(&aa, &bb);
        let e_a24 = fe_mul(&e, &a24);
        let aa_e_a24 = fe_add(&aa, &e_a24);
        z_2 = fe_mul(&e, &aa_e_a24);
    }

    // Final swap
    fe_cswap(&mut x_2, &mut x_3, swap);
    fe_cswap(&mut z_2, &mut z_3, swap);

    // Return x_2 * z_2^(-1)
    let z_inv = fe_invert(&z_2);
    let result = fe_mul(&x_2, &z_inv);
    fe_tobytes(&result)
}

/// Convenience: shared_secret = our_secret * their_public.
pub fn shared_secret(our_secret: &[u8; 32], their_public: &[u8; 32]) -> [u8; 32] {
    scalarmult(our_secret, their_public)
}

/// Generate a keypair: (secret_key, public_key) where public = secret * base_point(9).
pub fn keypair() -> crate::Result<([u8; 32], [u8; 32])> {
    let mut secret = [0u8; 32];
    super::getrandom(&mut secret)?;

    let base_point = {
        let mut bp = [0u8; 32];
        bp[0] = 9;
        bp
    };

    let public = scalarmult(&secret, &base_point);
    Ok((secret, public))
}
