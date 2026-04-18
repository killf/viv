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
