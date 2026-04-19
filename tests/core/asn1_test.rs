use viv::core::asn1::{Tag, TagClass};

#[test]
fn tag_class_discriminated() {
    assert_ne!(TagClass::Universal, TagClass::ContextSpecific);
}
