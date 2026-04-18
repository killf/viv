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
