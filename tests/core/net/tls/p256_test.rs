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
