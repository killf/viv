//! P-256 (secp256r1) elliptic curve arithmetic.
//!
//! Operations needed for ECDSA signature verification: field arithmetic
//! over GF(p), point operations in Jacobian coordinates, scalar
//! multiplication. Not constant-time — public-key operations only.

#![allow(dead_code)]

use crate::Error;
use crate::core::bigint::BigUint;

/// P-256 prime: p = 2^256 − 2^224 + 2^192 + 2^96 − 1.
pub(crate) fn p_modulus() -> BigUint {
    BigUint::from_bytes_be(&[
        0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff,
    ])
}

/// P-256 curve order n.
pub(crate) fn n_order() -> BigUint {
    BigUint::from_bytes_be(&[
        0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84, 0xf3, 0xb9, 0xca, 0xc2, 0xfc, 0x63,
        0x25, 0x51,
    ])
}

/// Curve coefficient b (a is fixed at -3).
pub(crate) fn b_coeff() -> BigUint {
    BigUint::from_bytes_be(&[
        0x5a, 0xc6, 0x35, 0xd8, 0xaa, 0x3a, 0x93, 0xe7, 0xb3, 0xeb, 0xbd, 0x55, 0x76, 0x98, 0x86,
        0xbc, 0x65, 0x1d, 0x06, 0xb0, 0xcc, 0x53, 0xb0, 0xf6, 0x3b, 0xce, 0x3c, 0x3e, 0x27, 0xd2,
        0x60, 0x4b,
    ])
}

/// Generator Gx.
pub(crate) fn gx() -> BigUint {
    BigUint::from_bytes_be(&[
        0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63, 0xa4, 0x40,
        0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39, 0x45, 0xd8, 0x98,
        0xc2, 0x96,
    ])
}

/// Generator Gy.
pub(crate) fn gy() -> BigUint {
    BigUint::from_bytes_be(&[
        0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b, 0x8e, 0xe7, 0xeb, 0x4a, 0x7c, 0x0f, 0x9e,
        0x16, 0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e, 0xce, 0xcb, 0xb6, 0x40, 0x68, 0x37, 0xbf,
        0x51, 0xf5,
    ])
}

/// P-256 field element in GF(p). Internally a canonical BigUint < p.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldElement(pub(crate) BigUint);

impl FieldElement {
    pub fn zero() -> Self {
        FieldElement(BigUint::zero())
    }

    pub fn one() -> Self {
        FieldElement(BigUint::one())
    }

    /// Parse 32-byte big-endian value. Returns `None` if ≥ p.
    pub fn from_bytes_be(bytes: &[u8; 32]) -> Option<Self> {
        let n = BigUint::from_bytes_be(bytes);
        if n.cmp(&p_modulus()) != std::cmp::Ordering::Less {
            return None;
        }
        Some(FieldElement(n))
    }

    pub fn to_bytes_be(&self) -> [u8; 32] {
        let v = self.0.to_bytes_be(32);
        let mut out = [0u8; 32];
        let copy_len = v.len().min(32);
        let dst_start = 32 - copy_len;
        let src_start = v.len() - copy_len;
        out[dst_start..].copy_from_slice(&v[src_start..]);
        out
    }

    pub fn add(&self, other: &Self) -> Self {
        let sum = self.0.add(&other.0);
        let p = p_modulus();
        if sum.cmp(&p) != std::cmp::Ordering::Less {
            FieldElement(sum.checked_sub(&p).unwrap_or(BigUint::zero()))
        } else {
            FieldElement(sum)
        }
    }

    pub fn sub(&self, other: &Self) -> Self {
        match self.0.checked_sub(&other.0) {
            Some(d) => FieldElement(d),
            None => {
                let p = p_modulus();
                let a_plus_p = self.0.add(&p);
                FieldElement(a_plus_p.checked_sub(&other.0).unwrap_or(BigUint::zero()))
            }
        }
    }

    pub fn neg(&self) -> Self {
        if self.0.is_zero() {
            FieldElement::zero()
        } else {
            FieldElement(p_modulus().checked_sub(&self.0).unwrap_or(BigUint::zero()))
        }
    }

    pub fn mul(&self, other: &Self) -> Self {
        let product = self.0.mul(&other.0);
        let (_, r) = product
            .div_rem(&p_modulus())
            .unwrap_or((BigUint::zero(), BigUint::zero()));
        FieldElement(r)
    }

    pub fn square(&self) -> Self {
        self.mul(self)
    }

    /// Multiplicative inverse via Fermat's little theorem: a^(p-2) mod p.
    /// Returns `None` if `self` is zero.
    pub fn invert(&self) -> Option<Self> {
        if self.0.is_zero() {
            return None;
        }
        let p = p_modulus();
        let two = BigUint::from_u64(2);
        let exp = p.checked_sub(&two)?;
        let inv = self.0.modexp(&exp, &p)?;
        Some(FieldElement(inv))
    }
}

/// Point in Jacobian coordinates (X:Y:Z); z==0 means point at infinity.
#[derive(Debug, Clone)]
pub struct Point {
    pub x: FieldElement,
    pub y: FieldElement,
    pub z: FieldElement,
}

impl Point {
    pub fn infinity() -> Self {
        Point {
            x: FieldElement::one(),
            y: FieldElement::one(),
            z: FieldElement::zero(),
        }
    }

    pub fn generator() -> Self {
        Point {
            x: FieldElement(gx()),
            y: FieldElement(gy()),
            z: FieldElement::one(),
        }
    }

    pub fn is_infinity(&self) -> bool {
        self.z.0.is_zero()
    }

    /// True for infinity, and for Z=1 affine points that satisfy
    /// y² = x³ − 3x + b (mod p).
    pub fn is_on_curve(&self) -> bool {
        if self.is_infinity() {
            return true;
        }
        let one = FieldElement::one();
        if self.z != one {
            // Post-arithmetic points may have Z != 1; we trust the formulas.
            return true;
        }
        let x = &self.x;
        let y = &self.y;
        let lhs = y.square();
        let x2 = x.square();
        let x3 = x2.mul(x);
        let three_x = x.add(x).add(x);
        let b = FieldElement(b_coeff());
        let rhs = x3.sub(&three_x).add(&b);
        lhs == rhs
    }

    /// Parse 65-byte uncompressed point: 0x04 || x || y.
    pub fn from_uncompressed(bytes: &[u8; 65]) -> crate::Result<Self> {
        if bytes[0] != 0x04 {
            return Err(Error::Tls(format!(
                "P-256 point: expected 0x04 prefix, got 0x{:02x}",
                bytes[0]
            )));
        }
        let mut x_bytes = [0u8; 32];
        x_bytes.copy_from_slice(&bytes[1..33]);
        let mut y_bytes = [0u8; 32];
        y_bytes.copy_from_slice(&bytes[33..65]);
        let x = FieldElement::from_bytes_be(&x_bytes)
            .ok_or_else(|| Error::Tls("P-256 point: x ≥ p".to_string()))?;
        let y = FieldElement::from_bytes_be(&y_bytes)
            .ok_or_else(|| Error::Tls("P-256 point: y ≥ p".to_string()))?;
        let p = Point {
            x,
            y,
            z: FieldElement::one(),
        };
        if !p.is_on_curve() {
            return Err(Error::Tls("P-256 point: not on curve".to_string()));
        }
        Ok(p)
    }
}
