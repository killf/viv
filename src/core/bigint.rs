//! Unsigned arbitrary-precision integer arithmetic.
//!
//! Used by the TLS 1.2 client for RSA public-key signature verification
//! and (later) P-256 ECDSA verification. Not constant-time — only suitable
//! for public-key operations.

use std::cmp::Ordering;

/// Unsigned big integer with little-endian u64 limbs.
///
/// Invariant: `limbs` has no trailing zeros. Zero is `limbs == vec![]`.
#[derive(Clone)]
pub struct BigUint {
    limbs: Vec<u64>,
}

impl BigUint {
    /// Zero constant.
    pub fn zero() -> Self {
        BigUint { limbs: Vec::new() }
    }
}
