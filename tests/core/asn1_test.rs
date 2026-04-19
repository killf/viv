use viv::core::asn1::{BitString, Parser, Tag, TagClass};

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

#[test]
fn read_any_simple_integer() {
    // 02 01 05 = INTEGER 5
    let mut p = Parser::new(&[0x02, 0x01, 0x05]);
    let (tag, value) = p.read_any().unwrap();
    assert_eq!(tag, Tag::INTEGER);
    assert_eq!(value, &[0x05]);
    assert!(p.is_empty());
}

#[test]
fn read_any_sequence_header() {
    // 30 03 02 01 05 = SEQUENCE { INTEGER 5 }
    let mut p = Parser::new(&[0x30, 0x03, 0x02, 0x01, 0x05]);
    let (tag, value) = p.read_any().unwrap();
    assert_eq!(tag, Tag::SEQUENCE);
    assert_eq!(value, &[0x02, 0x01, 0x05]);
    assert!(p.is_empty());
}

#[test]
fn read_any_truncated_value() {
    let mut p = Parser::new(&[0x04, 0x05, 0xaa, 0xbb]);
    assert!(p.read_any().is_err());
}

#[test]
fn read_expect_matches() {
    let mut p = Parser::new(&[0x02, 0x01, 0x42]);
    let value = p.read_expect(Tag::INTEGER).unwrap();
    assert_eq!(value, &[0x42]);
}

#[test]
fn read_expect_mismatches() {
    let mut p = Parser::new(&[0x04, 0x01, 0xaa]);
    assert!(p.read_expect(Tag::INTEGER).is_err());
}

#[test]
fn peek_tag_does_not_advance() {
    let mut p = Parser::new(&[0x02, 0x01, 0x05]);
    let t = p.peek_tag().unwrap();
    assert_eq!(t, Tag::INTEGER);
    let (tag, _) = p.read_any().unwrap();
    assert_eq!(tag, Tag::INTEGER);
}

#[test]
fn peek_tag_empty_is_err() {
    let p = Parser::new(&[]);
    assert!(p.peek_tag().is_err());
}

#[test]
fn finish_consumed_ok() {
    let mut p = Parser::new(&[0x02, 0x01, 0x05]);
    let _ = p.read_any().unwrap();
    assert!(p.finish().is_ok());
}

#[test]
fn finish_leftover_errors() {
    let mut p = Parser::new(&[0x02, 0x01, 0x05, 0xff]);
    let _ = p.read_any().unwrap();
    assert!(p.finish().is_err());
}

#[test]
fn read_sequence_yields_sub_parser() {
    // 30 06 02 01 01 02 01 02 = SEQUENCE { INTEGER 1, INTEGER 2 }
    let mut p = Parser::new(&[0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x02]);
    let mut seq = p.read_sequence().unwrap();
    assert_eq!(seq.read_expect(Tag::INTEGER).unwrap(), &[0x01]);
    assert_eq!(seq.read_expect(Tag::INTEGER).unwrap(), &[0x02]);
    assert!(seq.finish().is_ok());
    assert!(p.is_empty());
}

#[test]
fn read_sequence_wrong_tag_errors() {
    let mut p = Parser::new(&[0x04, 0x00]);
    assert!(p.read_sequence().is_err());
}

#[test]
fn read_set_yields_sub_parser() {
    // 31 03 02 01 07 = SET { INTEGER 7 }
    let mut p = Parser::new(&[0x31, 0x03, 0x02, 0x01, 0x07]);
    let mut s = p.read_set().unwrap();
    assert_eq!(s.read_expect(Tag::INTEGER).unwrap(), &[0x07]);
    assert!(s.finish().is_ok());
}

#[test]
fn read_explicit_unwraps() {
    // A0 03 02 01 05 = [0] EXPLICIT INTEGER 5
    let mut p = Parser::new(&[0xA0, 0x03, 0x02, 0x01, 0x05]);
    let mut inner = p.read_explicit(0).unwrap();
    assert_eq!(inner.read_expect(Tag::INTEGER).unwrap(), &[0x05]);
    assert!(inner.finish().is_ok());
}

#[test]
fn read_explicit_wrong_number_errors() {
    // A1 ... = [1] EXPLICIT; we ask for [0]
    let mut p = Parser::new(&[0xA1, 0x03, 0x02, 0x01, 0x05]);
    assert!(p.read_explicit(0).is_err());
}

#[test]
fn read_optional_present() {
    let mut p = Parser::new(&[0x02, 0x01, 0x05, 0x04, 0x01, 0xaa]);
    let value = p.read_optional(Tag::INTEGER).unwrap();
    assert_eq!(value, Some(&[0x05][..]));
    let v2 = p.read_expect(Tag::OCTET_STRING).unwrap();
    assert_eq!(v2, &[0xaa]);
}

#[test]
fn read_optional_absent_does_not_advance() {
    let mut p = Parser::new(&[0x04, 0x01, 0xaa]);
    let value = p.read_optional(Tag::INTEGER).unwrap();
    assert_eq!(value, None);
    let (tag, _) = p.read_any().unwrap();
    assert_eq!(tag, Tag::OCTET_STRING);
}

#[test]
fn read_optional_explicit_present() {
    let mut p = Parser::new(&[0xA0, 0x03, 0x02, 0x01, 0x05, 0x02, 0x01, 0x07]);
    let inner = p.read_optional_explicit(0).unwrap();
    assert!(inner.is_some());
    let mut inner = inner.unwrap();
    assert_eq!(inner.read_expect(Tag::INTEGER).unwrap(), &[0x05]);
    inner.finish().unwrap();
    assert_eq!(p.read_expect(Tag::INTEGER).unwrap(), &[0x07]);
}

#[test]
fn read_optional_explicit_absent() {
    let mut p = Parser::new(&[0x02, 0x01, 0x07]);
    let inner = p.read_optional_explicit(0).unwrap();
    assert!(inner.is_none());
    assert_eq!(p.read_expect(Tag::INTEGER).unwrap(), &[0x07]);
}

#[test]
fn read_optional_at_end_of_input() {
    let mut p = Parser::new(&[]);
    let v = p.read_optional(Tag::INTEGER).unwrap();
    assert_eq!(v, None);
}

#[test]
fn read_bool_true() {
    let mut p = Parser::new(&[0x01, 0x01, 0xff]);
    assert_eq!(p.read_bool().unwrap(), true);
}

#[test]
fn read_bool_false() {
    let mut p = Parser::new(&[0x01, 0x01, 0x00]);
    assert_eq!(p.read_bool().unwrap(), false);
}

#[test]
fn read_bool_rejects_wrong_length() {
    let mut p = Parser::new(&[0x01, 0x02, 0x00, 0x00]);
    assert!(p.read_bool().is_err());
}

#[test]
fn read_null_ok() {
    let mut p = Parser::new(&[0x05, 0x00]);
    assert!(p.read_null().is_ok());
    assert!(p.is_empty());
}

#[test]
fn read_null_rejects_nonzero_length() {
    let mut p = Parser::new(&[0x05, 0x01, 0x00]);
    assert!(p.read_null().is_err());
}

#[test]
fn read_integer_positive() {
    let mut p = Parser::new(&[0x02, 0x01, 0x05]);
    assert_eq!(p.read_integer().unwrap(), &[0x05]);
}

#[test]
fn read_integer_multi_byte() {
    let mut p = Parser::new(&[0x02, 0x02, 0x01, 0x00]);
    assert_eq!(p.read_integer().unwrap(), &[0x01, 0x00]);
}

#[test]
fn read_oid_simple() {
    let mut p = Parser::new(&[0x06, 0x03, 0x2a, 0x86, 0x48]);
    assert_eq!(p.read_oid().unwrap(), &[0x2a, 0x86, 0x48]);
}

#[test]
fn read_octet_string_basic() {
    let mut p = Parser::new(&[0x04, 0x04, 0xde, 0xad, 0xbe, 0xef]);
    assert_eq!(p.read_octet_string().unwrap(), &[0xde, 0xad, 0xbe, 0xef]);
}

#[test]
fn read_bit_string_no_unused() {
    let mut p = Parser::new(&[0x03, 0x04, 0x00, 0xab, 0xcd, 0xef]);
    let bs = p.read_bit_string().unwrap();
    assert_eq!(bs.unused_bits, 0);
    assert_eq!(bs.bytes, &[0xab, 0xcd, 0xef]);
}

#[test]
fn read_bit_string_with_unused() {
    let mut p = Parser::new(&[0x03, 0x04, 0x06, 0x01, 0x23, 0x45]);
    let bs = p.read_bit_string().unwrap();
    assert_eq!(bs.unused_bits, 6);
    assert_eq!(bs.bytes, &[0x01, 0x23, 0x45]);
}

#[test]
fn read_bit_string_rejects_empty_value() {
    let mut p = Parser::new(&[0x03, 0x00]);
    assert!(p.read_bit_string().is_err());
}

#[test]
fn read_bit_string_rejects_unused_over_7() {
    let mut p = Parser::new(&[0x03, 0x02, 0x08, 0xff]);
    assert!(p.read_bit_string().is_err());
}

// Suppress unused-import lint when BitString is only used via inference below.
#[allow(dead_code)]
fn _bitstring_used(_bs: BitString<'_>) {}
