use viv::core::asn1::{Parser, Tag, TagClass};

#[test]
fn tag_class_discriminated() {
    assert_ne!(TagClass::Universal, TagClass::ContextSpecific);
}

#[test]
fn tag_constants() {
    assert_eq!(Tag::INTEGER.number, 2);
    assert_eq!(Tag::INTEGER.class, TagClass::Universal);
    assert!(!Tag::INTEGER.constructed);

    assert_eq!(Tag::SEQUENCE.number, 16);
    assert!(Tag::SEQUENCE.constructed);
    assert_eq!(Tag::SET.number, 17);
    assert!(Tag::SET.constructed);

    assert_eq!(Tag::BOOLEAN.number, 1);
    assert_eq!(Tag::BIT_STRING.number, 3);
    assert_eq!(Tag::OCTET_STRING.number, 4);
    assert_eq!(Tag::NULL.number, 5);
    assert_eq!(Tag::OID.number, 6);
    assert_eq!(Tag::UTF8_STRING.number, 12);
    assert_eq!(Tag::PRINTABLE_STRING.number, 19);
    assert_eq!(Tag::IA5_STRING.number, 22);
    assert_eq!(Tag::UTC_TIME.number, 23);
    assert_eq!(Tag::GENERALIZED_TIME.number, 24);
    assert_eq!(Tag::BMP_STRING.number, 30);
}

#[test]
fn tag_from_bytes_short_form_primitive() {
    let (tag, consumed) = Tag::from_bytes(&[0x02]).unwrap();
    assert_eq!(tag, Tag::INTEGER);
    assert_eq!(consumed, 1);
}

#[test]
fn tag_from_bytes_short_form_constructed() {
    let (tag, consumed) = Tag::from_bytes(&[0x30]).unwrap();
    assert_eq!(tag, Tag::SEQUENCE);
    assert_eq!(consumed, 1);
}

#[test]
fn tag_from_bytes_context_specific() {
    let (tag, consumed) = Tag::from_bytes(&[0xA0]).unwrap();
    assert_eq!(tag.class, TagClass::ContextSpecific);
    assert!(tag.constructed);
    assert_eq!(tag.number, 0);
    assert_eq!(consumed, 1);
}

#[test]
fn tag_from_bytes_high_tag_number() {
    // 0x1f primitive universal, number = 128 (0x81 0x00 in base-128)
    let (tag, consumed) = Tag::from_bytes(&[0x1f, 0x81, 0x00]).unwrap();
    assert_eq!(tag.class, TagClass::Universal);
    assert!(!tag.constructed);
    assert_eq!(tag.number, 128);
    assert_eq!(consumed, 3);
}

#[test]
fn tag_from_bytes_empty_is_err() {
    assert!(Tag::from_bytes(&[]).is_err());
}

#[test]
fn tag_from_bytes_truncated_high_tag() {
    assert!(Tag::from_bytes(&[0x1f]).is_err());
    assert!(Tag::from_bytes(&[0x1f, 0x81]).is_err());
}

#[test]
fn tag_to_short_byte_roundtrip() {
    assert_eq!(Tag::INTEGER.to_short_byte(), Some(0x02));
    assert_eq!(Tag::SEQUENCE.to_short_byte(), Some(0x30));
    assert_eq!(Tag::SET.to_short_byte(), Some(0x31));
    let ctx = Tag::context(0, true);
    assert_eq!(ctx.to_short_byte(), Some(0xA0));
}

#[test]
fn tag_to_short_byte_none_for_high_tag_number() {
    let big = Tag {
        class: TagClass::Universal,
        constructed: false,
        number: 31,
    };
    assert_eq!(big.to_short_byte(), None);
}

#[test]
fn parser_new_and_empty() {
    let p = Parser::new(&[]);
    assert!(p.is_empty());
    assert_eq!(p.remaining(), &[] as &[u8]);
}

#[test]
fn parser_new_nonempty() {
    let p = Parser::new(&[0x30, 0x00]);
    assert!(!p.is_empty());
    assert_eq!(p.remaining(), &[0x30, 0x00]);
}

#[test]
fn read_length_short_form() {
    let mut p = Parser::new(&[0x05]);
    assert_eq!(p.read_length_for_test().unwrap(), 5);
    assert!(p.is_empty());
}

#[test]
fn read_length_long_form_1byte() {
    let mut p = Parser::new(&[0x81, 0xff]);
    assert_eq!(p.read_length_for_test().unwrap(), 255);
}

#[test]
fn read_length_long_form_2byte() {
    let mut p = Parser::new(&[0x82, 0x01, 0x00]);
    assert_eq!(p.read_length_for_test().unwrap(), 256);
}

#[test]
fn read_length_long_form_4byte() {
    let mut p = Parser::new(&[0x84, 0x00, 0x01, 0x00, 0x00]);
    assert_eq!(p.read_length_for_test().unwrap(), 65536);
}

#[test]
fn read_length_rejects_indefinite() {
    let mut p = Parser::new(&[0x80]);
    assert!(p.read_length_for_test().is_err());
}

#[test]
fn read_length_rejects_empty() {
    let mut p = Parser::new(&[]);
    assert!(p.read_length_for_test().is_err());
}

#[test]
fn read_length_rejects_truncated_long_form() {
    let mut p = Parser::new(&[0x82, 0x01]);
    assert!(p.read_length_for_test().is_err());
}

#[test]
fn read_length_rejects_over_4_bytes() {
    let mut p = Parser::new(&[0x85, 0x00, 0x00, 0x00, 0x00, 0x01]);
    assert!(p.read_length_for_test().is_err());
}
