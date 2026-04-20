use viv::core::encoding::base64;

#[test]
fn encode_empty() {
    assert_eq!(base64::encode(b""), "");
}
#[test]
fn encode_one_byte() {
    assert_eq!(base64::encode(b"M"), "TQ==");
}
#[test]
fn encode_two_bytes() {
    assert_eq!(base64::encode(b"Ma"), "TWE=");
}
#[test]
fn encode_three_bytes() {
    assert_eq!(base64::encode(b"Man"), "TWFu");
}
#[test]
fn encode_longer() {
    assert_eq!(base64::encode(b"Hello, World!"), "SGVsbG8sIFdvcmxkIQ==");
}
#[test]
fn decode_empty() {
    assert_eq!(base64::decode("").unwrap(), b"");
}
#[test]
fn decode_one_byte() {
    assert_eq!(base64::decode("TQ==").unwrap(), b"M");
}
#[test]
fn decode_two_bytes() {
    assert_eq!(base64::decode("TWE=").unwrap(), b"Ma");
}
#[test]
fn decode_three_bytes() {
    assert_eq!(base64::decode("TWFu").unwrap(), b"Man");
}
#[test]
fn decode_longer() {
    assert_eq!(
        base64::decode("SGVsbG8sIFdvcmxkIQ==").unwrap(),
        b"Hello, World!"
    );
}
#[test]
fn roundtrip() {
    let input = b"The quick brown fox jumps over the lazy dog";
    assert_eq!(base64::decode(&base64::encode(input)).unwrap(), input);
}
#[test]
fn decode_invalid_char() {
    assert!(base64::decode("TQ==!").is_err());
}
