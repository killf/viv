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

    /// One constant.
    pub fn one() -> Self {
        BigUint { limbs: vec![1] }
    }

    /// Construct from a single u64 value.
    pub fn from_u64(v: u64) -> Self {
        if v == 0 {
            Self::zero()
        } else {
            BigUint { limbs: vec![v] }
        }
    }

    /// True if this value is zero.
    pub fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    /// Compare two BigUints numerically.
    pub fn cmp(&self, other: &Self) -> Ordering {
        match self.limbs.len().cmp(&other.limbs.len()) {
            Ordering::Equal => {
                // Compare limb-by-limb from the highest.
                for (a, b) in self.limbs.iter().rev().zip(other.limbs.iter().rev()) {
                    match a.cmp(b) {
                        Ordering::Equal => continue,
                        non_eq => return non_eq,
                    }
                }
                Ordering::Equal
            }
            non_eq => non_eq,
        }
    }

    /// Construct from big-endian bytes. Leading zero bytes are stripped.
    pub fn from_bytes_be(bytes: &[u8]) -> Self {
        let start = bytes.iter().position(|&b| b != 0).unwrap_or(bytes.len());
        let trimmed = &bytes[start..];
        if trimmed.is_empty() {
            return Self::zero();
        }
        let byte_count = trimmed.len();
        let limb_count = byte_count.div_ceil(8);
        let mut limbs = vec![0u64; limb_count];
        for i in 0..byte_count {
            let b = trimmed[byte_count - 1 - i] as u64;
            limbs[i / 8] |= b << ((i % 8) * 8);
        }
        normalize(&mut limbs);
        BigUint { limbs }
    }

    /// Return big-endian byte representation, padded on the left to at least
    /// `out_len` bytes. If the value needs more bytes, no truncation happens.
    pub fn to_bytes_be(&self, out_len: usize) -> Vec<u8> {
        let bl = self.byte_len();
        let n = bl.max(out_len);
        let mut out = vec![0u8; n];
        for i in 0..bl {
            let limb = self.limbs[i / 8];
            let byte = ((limb >> ((i % 8) * 8)) & 0xff) as u8;
            out[n - 1 - i] = byte;
        }
        out
    }

    /// Number of bits needed to represent this value. 0 for zero.
    pub fn bit_len(&self) -> usize {
        match self.limbs.last() {
            None => 0,
            Some(&top) => (self.limbs.len() - 1) * 64 + (64 - top.leading_zeros() as usize),
        }
    }

    /// Number of bytes needed to represent this value. 0 for zero.
    fn byte_len(&self) -> usize {
        self.bit_len().div_ceil(8)
    }

    /// Addition: `self + other`. Never fails.
    pub fn add(&self, other: &Self) -> Self {
        let n = self.limbs.len().max(other.limbs.len());
        let mut out = Vec::with_capacity(n + 1);
        let mut carry: u64 = 0;
        for i in 0..n {
            let a = self.limbs.get(i).copied().unwrap_or(0);
            let b = other.limbs.get(i).copied().unwrap_or(0);
            let sum = (a as u128) + (b as u128) + (carry as u128);
            out.push(sum as u64);
            carry = (sum >> 64) as u64;
        }
        if carry != 0 {
            out.push(carry);
        }
        normalize(&mut out);
        BigUint { limbs: out }
    }
}

/// Strip trailing zero limbs so `BigUint` invariants hold.
fn normalize(limbs: &mut Vec<u64>) {
    while limbs.last() == Some(&0) {
        limbs.pop();
    }
}

impl PartialEq for BigUint {
    fn eq(&self, other: &Self) -> bool {
        self.limbs == other.limbs
    }
}

impl Eq for BigUint {}

impl std::fmt::Debug for BigUint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.limbs.is_empty() {
            return write!(f, "BigUint(0x0)");
        }
        write!(f, "BigUint(0x")?;
        // Highest limb: no leading zeros
        let mut first = true;
        for &limb in self.limbs.iter().rev() {
            if first {
                write!(f, "{:x}", limb)?;
                first = false;
            } else {
                write!(f, "{:016x}", limb)?;
            }
        }
        write!(f, ")")
    }
}
