use viv::core::net::tls::ecdsa::EcdsaPublicKey;

const SPKI_DER_HEX: &str = "\
3059301306072a8648ce3d020106082a8648ce3d0301070342000435c82a54589b8c2c\
da8c19e08edbd7a100639c08a5469ca988dfd1b1819edb910115fb64327f539cd0e10\
86da24f43c73d1bbba7cdc8a8765493504ba1c62ff3";

const SIG_DER_HEX: &str = "\
3045022100a6d42119904daca8153f28db6081fe32267eb9bf786065e9f8840b932ee\
e55d7022006093485d155a6ff37a696ea1b17650569827a035f2e50757f19214fac79\
2be1";

const MSG: &[u8] = b"hello world";

fn hex_decode(s: &str) -> Vec<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn ecdsa_types_compile() {
    let _: Option<EcdsaPublicKey> = None;
}

#[test]
fn test_vectors_sizes() {
    assert_eq!(hex_decode(SPKI_DER_HEX).len(), 91);
    assert_eq!(hex_decode(SIG_DER_HEX).len(), 71);
}
