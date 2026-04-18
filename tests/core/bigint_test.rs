use std::cmp::Ordering;

use viv::core::bigint::BigUint;

#[test]
fn zero_constructor_compiles() {
    let _z = BigUint::zero();
}

#[test]
fn zero_is_zero() {
    assert!(BigUint::zero().is_zero());
}

#[test]
fn one_is_not_zero() {
    assert!(!BigUint::one().is_zero());
}

#[test]
fn from_u64_zero_is_zero() {
    assert!(BigUint::from_u64(0).is_zero());
}

#[test]
fn from_u64_nonzero_is_not_zero() {
    assert!(!BigUint::from_u64(42).is_zero());
}

#[test]
fn equality_structural() {
    assert_eq!(BigUint::from_u64(7), BigUint::from_u64(7));
    assert_ne!(BigUint::from_u64(7), BigUint::from_u64(8));
    assert_eq!(BigUint::zero(), BigUint::from_u64(0));
}

#[test]
fn cmp_basic() {
    assert_eq!(BigUint::from_u64(3).cmp(&BigUint::from_u64(7)), Ordering::Less);
    assert_eq!(BigUint::from_u64(7).cmp(&BigUint::from_u64(7)), Ordering::Equal);
    assert_eq!(BigUint::from_u64(9).cmp(&BigUint::from_u64(7)), Ordering::Greater);
}

#[test]
fn from_bytes_be_empty_is_zero() {
    assert!(BigUint::from_bytes_be(&[]).is_zero());
}

#[test]
fn from_bytes_be_single_byte() {
    let n = BigUint::from_bytes_be(&[42]);
    assert_eq!(n, BigUint::from_u64(42));
}

#[test]
fn from_bytes_be_multi_byte() {
    // 0x0102 = 258
    let n = BigUint::from_bytes_be(&[0x01, 0x02]);
    assert_eq!(n, BigUint::from_u64(258));
}

#[test]
fn from_bytes_be_strips_leading_zeros() {
    let n = BigUint::from_bytes_be(&[0, 0, 1, 2]);
    assert_eq!(n, BigUint::from_u64(258));
}

#[test]
fn from_bytes_be_crosses_limb_boundary() {
    // 9 bytes BE = 0x01_02_03_04_05_06_07_08_09 (72 bits, top byte 0x01 has 1 bit => 65 bits)
    let bytes = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09];
    let n = BigUint::from_bytes_be(&bytes);
    assert_eq!(n.bit_len(), 65);
}

#[test]
fn to_bytes_be_roundtrip_small() {
    let original = vec![0x00u8, 0x01, 0x02, 0xff];
    let n = BigUint::from_bytes_be(&original);
    assert_eq!(n.to_bytes_be(4), vec![0x00, 0x01, 0x02, 0xff]);
}

#[test]
fn to_bytes_be_pad_left() {
    let bytes = BigUint::one().to_bytes_be(32);
    let mut expected = vec![0u8; 31];
    expected.push(0x01);
    assert_eq!(bytes, expected);
}

#[test]
fn to_bytes_be_no_truncation_when_out_len_smaller() {
    let n = BigUint::from_bytes_be(&[0x01, 0x02, 0x03, 0x04]);
    let out = n.to_bytes_be(2);
    assert_eq!(out, vec![0x01, 0x02, 0x03, 0x04]);
}

#[test]
fn to_bytes_be_zero_with_padding() {
    let out = BigUint::zero().to_bytes_be(8);
    assert_eq!(out, vec![0u8; 8]);
}

#[test]
fn to_bytes_be_zero_no_padding() {
    let out = BigUint::zero().to_bytes_be(0);
    assert_eq!(out, Vec::<u8>::new());
}

#[test]
fn bit_len_zero() {
    assert_eq!(BigUint::zero().bit_len(), 0);
}

#[test]
fn bit_len_one() {
    assert_eq!(BigUint::one().bit_len(), 1);
}

#[test]
fn bit_len_u64_max() {
    assert_eq!(BigUint::from_u64(u64::MAX).bit_len(), 64);
}

#[test]
fn bit_len_cross_limb() {
    // 2^64 encoded as 9 bytes BE: [0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
    let n = BigUint::from_bytes_be(&[0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    assert_eq!(n.bit_len(), 65);
}

#[test]
fn add_zero_identity() {
    let a = BigUint::from_u64(42);
    assert_eq!(a.add(&BigUint::zero()), a);
    assert_eq!(BigUint::zero().add(&a), a);
}

#[test]
fn add_simple() {
    let a = BigUint::from_u64(7);
    let b = BigUint::from_u64(35);
    assert_eq!(a.add(&b), BigUint::from_u64(42));
}

#[test]
fn add_carry_within_limb() {
    let a = BigUint::from_u64(u64::MAX);
    let b = BigUint::from_u64(1);
    let sum = a.add(&b);
    // 2^64
    assert_eq!(sum.bit_len(), 65);
    assert_eq!(sum.to_bytes_be(9), vec![1, 0, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn add_propagates_multi_limb_carry() {
    // [u64::MAX, u64::MAX] + 1 = [0, 0, 1] = 2^128
    let a = BigUint::from_bytes_be(&[
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff,
    ]);
    let b = BigUint::one();
    let sum = a.add(&b);
    assert_eq!(sum.bit_len(), 129);
    let mut expected = vec![0u8; 16];
    expected.insert(0, 1);
    assert_eq!(sum.to_bytes_be(17), expected);
}

#[test]
fn add_different_widths() {
    // 2^64 + 3
    let a = BigUint::from_u64(3);
    let b = BigUint::from_bytes_be(&[0x01, 0, 0, 0, 0, 0, 0, 0, 0]);
    let sum = a.add(&b);
    assert_eq!(sum.to_bytes_be(9), vec![1, 0, 0, 0, 0, 0, 0, 0, 3]);
}

#[test]
fn sub_equal_is_zero() {
    let a = BigUint::from_u64(42);
    assert_eq!(a.checked_sub(&a), Some(BigUint::zero()));
}

#[test]
fn sub_simple() {
    let a = BigUint::from_u64(50);
    let b = BigUint::from_u64(8);
    assert_eq!(a.checked_sub(&b), Some(BigUint::from_u64(42)));
}

#[test]
fn sub_underflow_returns_none() {
    let a = BigUint::from_u64(5);
    let b = BigUint::from_u64(10);
    assert_eq!(a.checked_sub(&b), None);
}

#[test]
fn sub_zero_identity() {
    let a = BigUint::from_u64(42);
    assert_eq!(a.checked_sub(&BigUint::zero()), Some(a));
}

#[test]
fn sub_borrow_cross_limb() {
    // 2^64 - 1 = u64::MAX
    let a = BigUint::from_bytes_be(&[1, 0, 0, 0, 0, 0, 0, 0, 0]);
    let b = BigUint::one();
    let diff = a.checked_sub(&b).unwrap();
    assert_eq!(diff, BigUint::from_u64(u64::MAX));
}

#[test]
fn sub_normalizes_result() {
    // 2^64 - (2^64 - 1) = 1
    let a = BigUint::from_bytes_be(&[1, 0, 0, 0, 0, 0, 0, 0, 0]);
    let b = BigUint::from_u64(u64::MAX);
    let diff = a.checked_sub(&b).unwrap();
    assert_eq!(diff, BigUint::one());
}

#[test]
fn mul_by_zero_is_zero() {
    let a = BigUint::from_u64(42);
    assert_eq!(a.mul(&BigUint::zero()), BigUint::zero());
    assert_eq!(BigUint::zero().mul(&a), BigUint::zero());
}

#[test]
fn mul_by_one_identity() {
    let a = BigUint::from_u64(42);
    assert_eq!(a.mul(&BigUint::one()), a);
}

#[test]
fn mul_small() {
    let a = BigUint::from_u64(7);
    let b = BigUint::from_u64(6);
    assert_eq!(a.mul(&b), BigUint::from_u64(42));
}

#[test]
fn mul_cross_limb() {
    // (2^32) * (2^32) = 2^64
    let a = BigUint::from_u64(1u64 << 32);
    let b = BigUint::from_u64(1u64 << 32);
    let p = a.mul(&b);
    assert_eq!(p.bit_len(), 65);
    assert_eq!(p.to_bytes_be(9), vec![1, 0, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn mul_u64_max_squared() {
    // (2^64 - 1)^2 = 2^128 - 2^65 + 1 = 0xfffffffffffffffe_0000000000000001
    let a = BigUint::from_u64(u64::MAX);
    let p = a.mul(&a);
    let expected = BigUint::from_bytes_be(&[
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x01,
    ]);
    assert_eq!(p, expected);
}

#[test]
fn mul_commutative() {
    let a = BigUint::from_bytes_be(&[1, 2, 3, 4, 5, 6, 7, 8, 9]);
    let b = BigUint::from_bytes_be(&[9, 8, 7, 6, 5, 4, 3, 2, 1]);
    assert_eq!(a.mul(&b), b.mul(&a));
}

#[test]
fn div_rem_by_zero_is_none() {
    let a = BigUint::from_u64(42);
    assert_eq!(a.div_rem(&BigUint::zero()), None);
}

#[test]
fn div_rem_zero_by_nonzero() {
    let (q, r) = BigUint::zero().div_rem(&BigUint::from_u64(7)).unwrap();
    assert_eq!(q, BigUint::zero());
    assert_eq!(r, BigUint::zero());
}

#[test]
fn div_rem_smaller_by_larger() {
    let a = BigUint::from_u64(5);
    let b = BigUint::from_u64(10);
    let (q, r) = a.div_rem(&b).unwrap();
    assert_eq!(q, BigUint::zero());
    assert_eq!(r, a);
}

#[test]
fn div_rem_exact() {
    let a = BigUint::from_u64(42);
    let b = BigUint::from_u64(6);
    let (q, r) = a.div_rem(&b).unwrap();
    assert_eq!(q, BigUint::from_u64(7));
    assert_eq!(r, BigUint::zero());
}

#[test]
fn div_rem_with_remainder() {
    let a = BigUint::from_u64(100);
    let b = BigUint::from_u64(7);
    let (q, r) = a.div_rem(&b).unwrap();
    assert_eq!(q, BigUint::from_u64(14));
    assert_eq!(r, BigUint::from_u64(2));
}

#[test]
fn div_rem_by_one() {
    let a = BigUint::from_u64(42);
    let (q, r) = a.div_rem(&BigUint::one()).unwrap();
    assert_eq!(q, a);
    assert_eq!(r, BigUint::zero());
}

#[test]
fn div_rem_roundtrip_multi_limb() {
    // q*d + r == a and r < b
    let a = BigUint::from_bytes_be(&[
        0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe, 0xba, 0xbe, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde,
        0xf0, 0x11, 0x22, 0x33, 0x44,
    ]);
    let b = BigUint::from_bytes_be(&[0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);
    let (q, r) = a.div_rem(&b).unwrap();
    assert_eq!(r.cmp(&b), Ordering::Less);
    let reconstructed = q.mul(&b).add(&r);
    assert_eq!(reconstructed, a);
}

#[test]
fn div_rem_large_divisor() {
    // 4096-bit / 2048-bit sanity
    let mut a_bytes = vec![0xabu8; 512];
    a_bytes[0] = 0x7f;
    let mut b_bytes = vec![0xcdu8; 256];
    b_bytes[0] = 0x7f;
    let a = BigUint::from_bytes_be(&a_bytes);
    let b = BigUint::from_bytes_be(&b_bytes);
    let (q, r) = a.div_rem(&b).unwrap();
    assert_eq!(r.cmp(&b), Ordering::Less);
    assert_eq!(q.mul(&b).add(&r), a);
}

#[test]
fn modexp_zero_modulus_is_none() {
    let a = BigUint::from_u64(5);
    let e = BigUint::from_u64(3);
    assert_eq!(a.modexp(&e, &BigUint::zero()), None);
}

#[test]
fn modexp_modulus_one_is_zero() {
    // Any x mod 1 == 0
    let a = BigUint::from_u64(42);
    let e = BigUint::from_u64(7);
    let m = BigUint::one();
    assert_eq!(a.modexp(&e, &m), Some(BigUint::zero()));
}

#[test]
fn modexp_zero_exp_is_one() {
    // x^0 mod m == 1 (for m > 1)
    let a = BigUint::from_u64(42);
    let e = BigUint::zero();
    let m = BigUint::from_u64(97);
    assert_eq!(a.modexp(&e, &m), Some(BigUint::one()));
}

#[test]
fn modexp_base_zero_is_zero() {
    // 0^e mod m == 0 (for e > 0)
    let a = BigUint::zero();
    let e = BigUint::from_u64(5);
    let m = BigUint::from_u64(97);
    assert_eq!(a.modexp(&e, &m), Some(BigUint::zero()));
}

#[test]
fn modexp_2_pow_10_mod_1000() {
    // 2^10 = 1024; 1024 mod 1000 = 24
    let a = BigUint::from_u64(2);
    let e = BigUint::from_u64(10);
    let m = BigUint::from_u64(1000);
    assert_eq!(a.modexp(&e, &m), Some(BigUint::from_u64(24)));
}

#[test]
fn modexp_3_pow_7_mod_11() {
    // 3^7 = 2187; 2187 mod 11 = 9
    let a = BigUint::from_u64(3);
    let e = BigUint::from_u64(7);
    let m = BigUint::from_u64(11);
    assert_eq!(a.modexp(&e, &m), Some(BigUint::from_u64(9)));
}

#[test]
fn modexp_fermat_little_theorem() {
    // For prime p, a^(p-1) mod p == 1.
    let a = BigUint::from_u64(5);
    let e = BigUint::from_u64(96);
    let m = BigUint::from_u64(97);
    assert_eq!(a.modexp(&e, &m), Some(BigUint::one()));
}
