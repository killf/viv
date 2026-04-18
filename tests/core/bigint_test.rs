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
