use viv::core::net::tls::p256::{FieldElement, Point};

#[test]
fn p256_types_compile() {
    let _: Option<FieldElement> = None;
    let _: Option<Point> = None;
}

#[test]
fn field_element_zero_and_one() {
    assert_ne!(FieldElement::zero(), FieldElement::one());
}

#[test]
fn field_element_roundtrip_small() {
    let mut bytes = [0u8; 32];
    bytes[31] = 42;
    let fe = FieldElement::from_bytes_be(&bytes).unwrap();
    assert_eq!(fe.to_bytes_be(), bytes);
}

#[test]
fn field_element_roundtrip_one() {
    let mut bytes = [0u8; 32];
    bytes[31] = 1;
    let fe = FieldElement::from_bytes_be(&bytes).unwrap();
    assert_eq!(fe, FieldElement::one());
    assert_eq!(fe.to_bytes_be(), bytes);
}

#[test]
fn field_element_rejects_value_ge_p() {
    let bytes = [0xff; 32];
    assert!(FieldElement::from_bytes_be(&bytes).is_none());
}

/// Build FieldElement from a small integer (LSB in bytes[31]).
fn fe_small(v: u64) -> FieldElement {
    let mut bytes = [0u8; 32];
    bytes[24..32].copy_from_slice(&v.to_be_bytes());
    FieldElement::from_bytes_be(&bytes).unwrap()
}

/// p - 1 as FieldElement (for wrap-around tests).
fn fe_p_minus_one() -> FieldElement {
    // p = ff ff ff ff 00 00 00 01 00 00 00 00 00 00 00 00 00 00 00 00 ff ff ff ff ff ff ff ff ff ff ff ff
    let mut bytes = [0u8; 32];
    bytes[0..4].copy_from_slice(&[0xff, 0xff, 0xff, 0xff]);
    bytes[4..8].copy_from_slice(&[0, 0, 0, 1]);
    // 8..20 zero
    bytes[20..32].copy_from_slice(&[0xff; 12]);
    bytes[31] = 0xfe; // p-1
    FieldElement::from_bytes_be(&bytes).unwrap()
}

#[test]
fn field_add_identity() {
    let one = FieldElement::one();
    assert_eq!(one.add(&FieldElement::zero()), one);
}

#[test]
fn field_add_basic() {
    let a = fe_small(2);
    let b = fe_small(3);
    assert_eq!(a.add(&b), fe_small(5));
}

#[test]
fn field_add_wraps_around_p() {
    let a = fe_p_minus_one();
    assert_eq!(a.add(&FieldElement::one()), FieldElement::zero());
}

#[test]
fn field_sub_basic() {
    let a = fe_small(3);
    let b = fe_small(2);
    assert_eq!(a.sub(&b), FieldElement::one());
}

#[test]
fn field_sub_wraps_when_underflow() {
    // 0 - 1 = p - 1
    assert_eq!(FieldElement::zero().sub(&FieldElement::one()), fe_p_minus_one());
}

#[test]
fn field_neg_zero_is_zero() {
    assert_eq!(FieldElement::zero().neg(), FieldElement::zero());
}

#[test]
fn field_neg_involution() {
    let a = FieldElement::one();
    assert_eq!(a.neg().neg(), a);
}

#[test]
fn field_mul_by_zero() {
    let a = fe_small(7);
    assert_eq!(a.mul(&FieldElement::zero()), FieldElement::zero());
}

#[test]
fn field_mul_by_one() {
    let a = fe_small(7);
    assert_eq!(a.mul(&FieldElement::one()), a);
}

#[test]
fn field_mul_small() {
    let a = fe_small(6);
    let b = fe_small(7);
    assert_eq!(a.mul(&b), fe_small(42));
}

#[test]
fn field_mul_p_minus_one_squared_is_one() {
    // (-1)^2 = 1
    let a = fe_p_minus_one();
    assert_eq!(a.mul(&a), FieldElement::one());
}
