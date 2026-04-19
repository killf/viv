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

#[test]
fn field_invert_zero_is_none() {
    assert!(FieldElement::zero().invert().is_none());
}

#[test]
fn field_invert_one() {
    assert_eq!(FieldElement::one().invert().unwrap(), FieldElement::one());
}

#[test]
fn field_invert_self_multiply_is_one() {
    let a = fe_small(0x1234);
    let inv = a.invert().unwrap();
    assert_eq!(a.mul(&inv), FieldElement::one());
}

/// 65-byte uncompressed encoding of generator G.
fn generator_bytes() -> [u8; 65] {
    let mut bytes = [0u8; 65];
    bytes[0] = 0x04;
    bytes[1..33].copy_from_slice(&[
        0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63, 0xa4, 0x40,
        0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39, 0x45, 0xd8, 0x98,
        0xc2, 0x96,
    ]);
    bytes[33..65].copy_from_slice(&[
        0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b, 0x8e, 0xe7, 0xeb, 0x4a, 0x7c, 0x0f, 0x9e,
        0x16, 0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e, 0xce, 0xcb, 0xb6, 0x40, 0x68, 0x37, 0xbf,
        0x51, 0xf5,
    ]);
    bytes
}

#[test]
fn point_infinity() {
    assert!(Point::infinity().is_infinity());
}

#[test]
fn point_generator_on_curve() {
    let g = Point::generator();
    assert!(!g.is_infinity());
    assert!(g.is_on_curve());
}

#[test]
fn point_from_uncompressed_generator() {
    let bytes = generator_bytes();
    let p = Point::from_uncompressed(&bytes).unwrap();
    assert!(p.is_on_curve());
    assert!(!p.is_infinity());
}

#[test]
fn point_from_uncompressed_rejects_compressed_prefix() {
    let mut bytes = [0u8; 65];
    bytes[0] = 0x02;
    assert!(Point::from_uncompressed(&bytes).is_err());
}

#[test]
fn point_from_uncompressed_rejects_off_curve() {
    let mut bytes = [0u8; 65];
    bytes[0] = 0x04;
    // x = y = 0; not on y² = x³ - 3x + b.
    assert!(Point::from_uncompressed(&bytes).is_err());
}

#[test]
fn point_add_infinity_is_identity() {
    let g = Point::generator();
    let inf = Point::infinity();
    let sum = g.add(&inf);
    assert!(!sum.is_infinity());
    assert_eq!(sum.affine_x_bytes().unwrap(), g.affine_x_bytes().unwrap());
}

#[test]
fn point_double_infinity_is_infinity() {
    assert!(Point::infinity().double().is_infinity());
}

#[test]
fn point_add_self_equals_double() {
    let g = Point::generator();
    let g_plus_g = g.add(&g);
    let two_g = g.double();
    assert_eq!(g_plus_g.affine_x_bytes(), two_g.affine_x_bytes());
}

#[test]
fn point_add_negation_is_infinity() {
    let g = Point::generator();
    let neg_g = Point {
        x: g.x.clone(),
        y: g.y.neg(),
        z: g.z.clone(),
    };
    let sum = g.add(&neg_g);
    assert!(sum.is_infinity());
}

#[test]
fn point_affine_x_of_infinity_is_none() {
    assert!(Point::infinity().affine_x_bytes().is_none());
}
