use viv::core::crypto::md5;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[test]
fn md5_empty() {
    assert_eq!(hex(&md5::md5(b"")), "d41d8cd98f00b204e9800998ecf8427e");
}
#[test]
fn md5_hello() {
    assert_eq!(hex(&md5::md5(b"hello")), "5d41402abc4b2a76b9719d911017c592");
}
#[test]
fn md5_abc() {
    assert_eq!(hex(&md5::md5(b"abc")), "900150983cd24fb0d6963f7d28e17f72");
}
#[test]
fn md5_long() {
    assert_eq!(
        hex(&md5::md5(b"The quick brown fox jumps over the lazy dog")),
        "9e107d9d372bb6826bd81d3542a419d6"
    );
}
