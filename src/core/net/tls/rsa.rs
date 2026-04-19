//! RSA PKCS#1 v1.5 signature verification (SHA-256 only).
//!
//! Verifies signatures produced by the private half of an RSA key pair.
//! Only public-key operations — not constant-time, not suitable for
//! private-key operations.

#![allow(dead_code)]

use crate::core::bigint::BigUint;

/// RSA public key: modulus `n` and exponent `e`.
pub struct RsaPublicKey {
    pub n: BigUint,
    pub e: BigUint,
}

impl RsaPublicKey {
    /// Construct from raw big-endian bytes.
    pub fn from_n_e(n_be: &[u8], e_be: &[u8]) -> Self {
        RsaPublicKey {
            n: BigUint::from_bytes_be(n_be),
            e: BigUint::from_bytes_be(e_be),
        }
    }

    /// Modulus byte length. Signatures must match this.
    pub fn n_byte_len(&self) -> usize {
        self.n.bit_len().div_ceil(8)
    }
}
