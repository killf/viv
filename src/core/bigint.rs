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

    /// Subtraction: `self - other`. Returns `None` if `self < other`.
    pub fn checked_sub(&self, other: &Self) -> Option<Self> {
        if self.cmp(other) == Ordering::Less {
            return None;
        }
        let n = self.limbs.len();
        let mut out = Vec::with_capacity(n);
        let mut borrow: i64 = 0;
        for i in 0..n {
            let a = self.limbs[i] as i128;
            let b = other.limbs.get(i).copied().unwrap_or(0) as i128;
            let diff = a - b - (borrow as i128);
            if diff < 0 {
                out.push((diff + (1i128 << 64)) as u64);
                borrow = 1;
            } else {
                out.push(diff as u64);
                borrow = 0;
            }
        }
        normalize(&mut out);
        Some(BigUint { limbs: out })
    }

    /// Multiplication: `self * other`. Schoolbook O(n²).
    pub fn mul(&self, other: &Self) -> Self {
        if self.is_zero() || other.is_zero() {
            return Self::zero();
        }
        let m = self.limbs.len();
        let n = other.limbs.len();
        let mut out = vec![0u64; m + n];
        for i in 0..m {
            let mut carry: u64 = 0;
            let a = self.limbs[i] as u128;
            for j in 0..n {
                let b = other.limbs[j] as u128;
                let cur = out[i + j] as u128;
                let prod = a * b + cur + carry as u128;
                out[i + j] = prod as u64;
                carry = (prod >> 64) as u64;
            }
            out[i + n] = out[i + n].wrapping_add(carry);
        }
        normalize(&mut out);
        BigUint { limbs: out }
    }

    /// Divide and take remainder: `(self / divisor, self % divisor)`.
    /// Returns `None` if `divisor` is zero.
    ///
    /// Bit-by-bit binary long division, MSB-first.
    pub fn div_rem(&self, divisor: &Self) -> Option<(Self, Self)> {
        if divisor.is_zero() {
            return None;
        }
        if self.cmp(divisor) == Ordering::Less {
            return Some((Self::zero(), self.clone()));
        }

        let n_bits = self.bit_len();
        let mut q = BigUint { limbs: Vec::new() };
        let mut r = BigUint { limbs: Vec::new() };

        for i in (0..n_bits).rev() {
            // r = r << 1
            shl1_in_place(&mut r.limbs);
            // r.bit[0] = self.bit[i]
            let bit = (self.limbs[i / 64] >> (i % 64)) & 1;
            if bit == 1 {
                if r.limbs.is_empty() {
                    r.limbs.push(1);
                } else {
                    r.limbs[0] |= 1;
                }
            }

            // if r >= divisor: r -= divisor; q.bit[i] = 1
            if r.cmp(divisor) != Ordering::Less {
                // The `?` will never fire here by the cmp check above, but we
                // use it to avoid unwrap() per the project's no-panic rule.
                r = r.checked_sub(divisor)?;
                set_bit(&mut q.limbs, i);
            }
        }

        normalize(&mut q.limbs);
        normalize(&mut r.limbs);
        Some((q, r))
    }

    /// Modular exponentiation: `self^exp mod modulus`.
    /// Returns `None` if `modulus` is zero. Left-to-right binary
    /// square-and-multiply.
    pub fn modexp(&self, exp: &Self, modulus: &Self) -> Option<Self> {
        if modulus.is_zero() {
            return None;
        }
        if modulus == &Self::one() {
            return Some(Self::zero());
        }
        if exp.is_zero() {
            return Some(Self::one());
        }

        // Reduce base mod modulus once up front.
        let (_, base_mod) = self.div_rem(modulus)?;

        let mut result = Self::one();
        let exp_bits = exp.bit_len();
        for i in (0..exp_bits).rev() {
            // Square
            result = result.mul(&result);
            let (_, r) = result.div_rem(modulus)?;
            result = r;
            // Multiply by base if exponent bit i is set
            let bit = (exp.limbs[i / 64] >> (i % 64)) & 1;
            if bit == 1 {
                result = result.mul(&base_mod);
                let (_, r) = result.div_rem(modulus)?;
                result = r;
            }
        }
        Some(result)
    }

    /// Modular inverse via the extended Euclidean algorithm with signed
    /// coefficients represented as `(BigUint, bool)` (bool = true means negative).
    ///
    /// Returns `Some(x)` such that `(self * x) mod modulus == 1`, or `None`
    /// when `gcd(self, modulus) != 1`, `modulus` is zero, or `modulus` is one.
    pub fn mod_inverse(&self, modulus: &Self) -> Option<Self> {
        if modulus.is_zero() || modulus == &Self::one() {
            return None;
        }
        let (_, a0) = self.div_rem(modulus)?;
        if a0.is_zero() {
            return None;
        }

        let mut old_r = a0;
        let mut r = modulus.clone();
        let mut old_s: (BigUint, bool) = (Self::one(), false);
        let mut s: (BigUint, bool) = (Self::zero(), false);

        while !r.is_zero() {
            let (q, new_r) = old_r.div_rem(&r)?;
            old_r = r;
            r = new_r;
            let q_s = signed_mul(&q, &s);
            let next_s = signed_sub(&old_s, &q_s);
            old_s = s;
            s = next_s;
        }

        if old_r != Self::one() {
            return None;
        }

        if old_s.1 {
            let (_, reduced) = old_s.0.div_rem(modulus)?;
            if reduced.is_zero() {
                Some(Self::zero())
            } else {
                modulus.checked_sub(&reduced)
            }
        } else {
            let (_, reduced) = old_s.0.div_rem(modulus)?;
            Some(reduced)
        }
    }
}

/// Signed multiply helper for mod_inverse: (|a|, _) * (|b|, sign_b) = (|a|*|b|, sign_b).
fn signed_mul(a: &BigUint, b: &(BigUint, bool)) -> (BigUint, bool) {
    (a.mul(&b.0), b.1)
}

/// Signed subtract helper for mod_inverse: (a, a_sign) - (b, b_sign).
fn signed_sub(a: &(BigUint, bool), b: &(BigUint, bool)) -> (BigUint, bool) {
    match (a.1, b.1) {
        (false, false) => {
            if a.0.cmp(&b.0) != Ordering::Less {
                (a.0.checked_sub(&b.0).unwrap_or(BigUint::zero()), false)
            } else {
                (b.0.checked_sub(&a.0).unwrap_or(BigUint::zero()), true)
            }
        }
        (false, true) => (a.0.add(&b.0), false),
        (true, false) => (a.0.add(&b.0), true),
        (true, true) => {
            if b.0.cmp(&a.0) != Ordering::Less {
                (b.0.checked_sub(&a.0).unwrap_or(BigUint::zero()), false)
            } else {
                (a.0.checked_sub(&b.0).unwrap_or(BigUint::zero()), true)
            }
        }
    }
}

/// Shift `limbs` left by one bit in place.
fn shl1_in_place(limbs: &mut Vec<u64>) {
    let mut carry = 0u64;
    for limb in limbs.iter_mut() {
        let new_carry = *limb >> 63;
        *limb = (*limb << 1) | carry;
        carry = new_carry;
    }
    if carry != 0 {
        limbs.push(carry);
    }
}

/// Set bit `i` (0-indexed from LSB) in a limb slice, growing if needed.
fn set_bit(limbs: &mut Vec<u64>, i: usize) {
    let limb_idx = i / 64;
    let bit_idx = i % 64;
    while limbs.len() <= limb_idx {
        limbs.push(0);
    }
    limbs[limb_idx] |= 1u64 << bit_idx;
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
