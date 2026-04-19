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
