//! ECDSA signature verification over the P-256 curve, SHA-256 digest.
//!
//! Only verification; no key generation or signing. Not constant-time.

#![allow(dead_code)]

use crate::Error;
use crate::core::net::tls::p256::Point;

pub struct EcdsaPublicKey {
    pub point: Point,
}

/// OID 1.2.840.10045.2.1 id-ecPublicKey.
const OID_EC_PUBLIC_KEY: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01];
/// OID 1.2.840.10045.3.1.7 prime256v1 / secp256r1.
const OID_SECP256R1: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07];

impl EcdsaPublicKey {
    /// Parse SubjectPublicKeyInfo DER for an EC public key on P-256.
    pub fn from_spki(der: &[u8]) -> crate::Result<Self> {
        use crate::core::crypto::asn1::Parser;
        let mut top = Parser::new(der);
        let mut spki = top
            .read_sequence()
            .map_err(|e| Error::Tls(format!("ECDSA SPKI: {e}")))?;

        let mut alg = spki
            .read_sequence()
            .map_err(|e| Error::Tls(format!("ECDSA alg: {e}")))?;
        let oid = alg
            .read_oid()
            .map_err(|e| Error::Tls(format!("ECDSA alg OID: {e}")))?;
        if oid != OID_EC_PUBLIC_KEY {
            return Err(Error::Tls(format!(
                "ECDSA SPKI: expected id-ecPublicKey, got {}-byte OID",
                oid.len()
            )));
        }
        let curve_oid = alg
            .read_oid()
            .map_err(|e| Error::Tls(format!("ECDSA curve OID: {e}")))?;
        if curve_oid != OID_SECP256R1 {
            return Err(Error::Tls(format!(
                "ECDSA SPKI: expected secp256r1, got {}-byte OID",
                curve_oid.len()
            )));
        }
        while !alg.is_empty() {
            let _ = alg
                .read_any()
                .map_err(|e| Error::Tls(format!("ECDSA alg extra: {e}")))?;
        }

        let bits = spki
            .read_bit_string()
            .map_err(|e| Error::Tls(format!("ECDSA bit string: {e}")))?;
        if bits.unused_bits != 0 {
            return Err(Error::Tls(format!(
                "ECDSA SPKI: BIT STRING unused_bits = {}",
                bits.unused_bits
            )));
        }
        if bits.bytes.len() != 65 {
            return Err(Error::Tls(format!(
                "ECDSA SPKI: expected 65-byte point, got {}",
                bits.bytes.len()
            )));
        }
        let mut point_bytes = [0u8; 65];
        point_bytes.copy_from_slice(bits.bytes);
        let point = Point::from_uncompressed(&point_bytes)?;
        Ok(EcdsaPublicKey { point })
    }
}

/// Verify ECDSA-SHA256 signature. `signature` is DER `SEQUENCE { r, s }`.
pub fn verify_ecdsa_sha256(
    pk: &EcdsaPublicKey,
    msg: &[u8],
    signature: &[u8],
) -> crate::Result<()> {
    use crate::core::crypto::sha256::Sha256;
    let digest = Sha256::hash(msg);
    verify_ecdsa_sha256_prehashed(pk, &digest, signature)
}

/// Same as `verify_ecdsa_sha256` but accepts a precomputed SHA-256 digest.
pub fn verify_ecdsa_sha256_prehashed(
    pk: &EcdsaPublicKey,
    digest: &[u8; 32],
    signature: &[u8],
) -> crate::Result<()> {
    use crate::core::crypto::asn1::Parser;
    use crate::core::crypto::bigint::BigUint;
    use crate::core::net::tls::p256::{Point, n_order};

    // 1. DER-decode signature into r, s.
    let mut parser = Parser::new(signature);
    let mut seq = parser
        .read_sequence()
        .map_err(|e| Error::Tls(format!("ECDSA sig seq: {e}")))?;
    let r_bytes = seq
        .read_integer()
        .map_err(|e| Error::Tls(format!("ECDSA r: {e}")))?;
    let s_bytes = seq
        .read_integer()
        .map_err(|e| Error::Tls(format!("ECDSA s: {e}")))?;
    seq.finish()
        .map_err(|e| Error::Tls(format!("ECDSA sig trailing: {e}")))?;
    let r = BigUint::from_bytes_be(r_bytes);
    let s = BigUint::from_bytes_be(s_bytes);

    // 2. Range check 1 ≤ r, s < n.
    let n = n_order();
    if r.is_zero() || r.cmp(&n) != std::cmp::Ordering::Less {
        return Err(Error::Tls("ECDSA: r out of range".to_string()));
    }
    if s.is_zero() || s.cmp(&n) != std::cmp::Ordering::Less {
        return Err(Error::Tls("ECDSA: s out of range".to_string()));
    }

    // 3. e = digest as integer (SHA-256 is 256 bits, same as n bit-width).
    let e = BigUint::from_bytes_be(digest);

    // 4. w = s^-1 mod n.
    let w = s
        .mod_inverse(&n)
        .ok_or_else(|| Error::Tls("ECDSA: s has no inverse mod n".to_string()))?;

    // 5. u1 = e*w mod n, u2 = r*w mod n.
    let (_, u1) = e
        .mul(&w)
        .div_rem(&n)
        .ok_or_else(|| Error::Tls("ECDSA: u1 reduction failed".to_string()))?;
    let (_, u2) = r
        .mul(&w)
        .div_rem(&n)
        .ok_or_else(|| Error::Tls("ECDSA: u2 reduction failed".to_string()))?;

    // 6. P = u1·G + u2·Q.
    let u1_bytes = big_to_32_be(&u1);
    let u2_bytes = big_to_32_be(&u2);

    let g = Point::generator();
    let u1g = g.scalar_mul(&u1_bytes);
    let u2q = pk.point.scalar_mul(&u2_bytes);
    let sum = u1g.add(&u2q);
    if sum.is_infinity() {
        return Err(Error::Tls("ECDSA: u1·G + u2·Q = infinity".to_string()));
    }

    // 7. v = x_P mod n; verify v == r.
    let x_bytes = sum
        .affine_x_bytes()
        .ok_or_else(|| Error::Tls("ECDSA: failed to extract affine x".to_string()))?;
    let x_big = BigUint::from_bytes_be(&x_bytes);
    let (_, v) = x_big
        .div_rem(&n)
        .ok_or_else(|| Error::Tls("ECDSA: x mod n failed".to_string()))?;
    if v == r {
        Ok(())
    } else {
        Err(Error::Tls("ECDSA: signature mismatch".to_string()))
    }
}

/// Encode a BigUint into exactly 32 big-endian bytes (left-padded with zeros).
fn big_to_32_be(n: &crate::core::crypto::bigint::BigUint) -> [u8; 32] {
    let v = n.to_bytes_be(32);
    let mut out = [0u8; 32];
    let copy_len = v.len().min(32);
    let dst_start = 32 - copy_len;
    let src_start = v.len() - copy_len;
    out[dst_start..].copy_from_slice(&v[src_start..]);
    out
}
