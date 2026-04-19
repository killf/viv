use viv::core::net::tls::rsa::RsaPublicKey;

const SPKI_DER_HEX: &str = "\
30820122300d06092a864886f70d01010105000382010f003082010a0282010100\
9eeb3ab702c7f02188c8739e8e3940f35d8bb5791c8dfa93e6a9a2e7da28e765ed\
3ed50ff4526082818e63f299899eccaadf826fd084fcf284ca1ca55efa3986cb23\
db0c8407c110ea30f321c64d1ef42db2bb3d6c86baa2cbe3830f8a9213799ca38d\
4e48fa0f4bdef72ad2ef5a7cf8bb3a01b52965394f79a474c15a03296a18512214\
e5c832003653eb0a34b4d2637ce3f754312ed361a3c8797c5c0e45ec1ff39d0b6c\
fa7767daa1aef6e95ec1a169dc5ec5efe1ae6ce4f61e67c8a4d57061b68fbaf6fa\
b2073045c9065a9bb3bf8275e80ba06399ed050875fa8b0fb1b90e8ee04ecf54ea\
a0a417a8157928fe45baa188c7e126f84411233967c5f406bb0203010001";

const SIG_HEX: &str = "\
985b94e8d051b590d57ed17b0a3c5a39276dabfe6117e08bd20cae38c38b0f99cf\
ee90f9d9e4b343e6617938d0234ca0babbfc39e7802545c405117ef0a0211a6e88\
1f5e48956ed7991c506f8e050a4b7a155004eafdf3e79ced24d5b6b662c76ffd3d\
8d4fa96f100a1ab71b48229cd1e6588905ce2d1a3e9196489162eb43d706ae2188\
b5a6051794a9ef0fae78e8cc6bde62226a00971288028ee9686dfede1a931226bd\
0815034f7d568f99fa0a62b23cf4a8c4c9db8bfb5a0c7b4692636757b2dc886c56\
7894bd5d31569c20b45b0b3c6ca8a78016c690384de30729d764c532cec87b7a29\
ced175776811d52cb4dc6aa746f6d450b7ed45e1b30f1a361f";

const MSG: &[u8] = b"hello world";

const MSG_SHA256_HEX: &str =
    "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";

fn hex_decode(s: &str) -> Vec<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn hex_decode_roundtrip() {
    assert_eq!(hex_decode("00ff"), vec![0x00, 0xff]);
    assert_eq!(hex_decode("de ad\nbe ef"), vec![0xde, 0xad, 0xbe, 0xef]);
}

#[test]
fn test_vectors_sizes_sanity() {
    assert_eq!(hex_decode(SIG_HEX).len(), 256);
    assert_eq!(hex_decode(MSG_SHA256_HEX).len(), 32);
    assert!(hex_decode(SPKI_DER_HEX).len() > 270);
}

#[test]
fn smoke_rsa_pk_constructs() {
    let pk = RsaPublicKey::from_n_e(&[0x01, 0x00, 0x01], &[0x03]);
    assert!(pk.n_byte_len() > 0);
}

#[test]
fn from_n_e_basic() {
    let pk = RsaPublicKey::from_n_e(&[0xff], &[0x03]);
    assert_eq!(pk.n_byte_len(), 1);
}

#[test]
fn from_n_e_2048_bit() {
    let mut n = vec![0xffu8; 256];
    n[0] = 0x80;
    let pk = RsaPublicKey::from_n_e(&n, &[0x01, 0x00, 0x01]);
    assert_eq!(pk.n_byte_len(), 256);
}

#[test]
fn from_n_e_strips_leading_zeros() {
    let pk = RsaPublicKey::from_n_e(&[0x00, 0x00, 0x01, 0x23], &[0x03]);
    assert_eq!(pk.n_byte_len(), 2);
}
