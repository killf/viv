use viv::qrcode::encode;

#[test]
fn select_version_1_byte() {
    assert_eq!(encode::select_version(1), Some(1));
}

#[test]
fn select_version_14_bytes() {
    assert_eq!(encode::select_version(14), Some(1)); // V1-M capacity = 14
}

#[test]
fn select_version_15_bytes() {
    assert_eq!(encode::select_version(15), Some(2)); // exceeds V1
}

#[test]
fn select_version_too_large() {
    assert_eq!(encode::select_version(3000), None);
}

#[test]
fn encode_data_hi() {
    let (codewords, version) = encode::encode_data("Hi").unwrap();
    assert_eq!(version, 1);
    assert_eq!(codewords.len(), 16); // V1-M total data = 16
    // Bit stream: 0100 (mode=byte) + 00000010 (count=2) + 01001000 ('H') + 01101001 ('i')
    // = 0100_0000_0010_0100_1000_0110_1001 ...
    // First byte: 0100_0000 = 0x40
    assert_eq!(codewords[0] & 0xF0, 0x40); // mode indicator 0100 in high nibble
}

#[test]
fn encode_data_empty_errors() {
    assert!(encode::encode_data("").is_err());
}

#[test]
fn encode_and_interleave_hello() {
    let result = encode::encode_and_interleave("HELLO").unwrap();
    assert_eq!(result.version, 1);
    // V1-M: 16 data + 10 ECC = 26 total codewords
    assert_eq!(result.data.len(), 26);
}

#[test]
fn encode_and_interleave_long_url() {
    let result = encode::encode_and_interleave("https://www.example.com/very/long/path/here").unwrap();
    assert!(result.version >= 3);
    assert!(!result.data.is_empty());
}
