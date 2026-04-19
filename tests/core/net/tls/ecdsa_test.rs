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

#[test]
fn from_spki_real_ec_key() {
    let der = hex_decode(SPKI_DER_HEX);
    let pk = EcdsaPublicKey::from_spki(&der).unwrap();
    assert!(!pk.point.is_infinity());
    assert!(pk.point.is_on_curve());
}

#[test]
fn from_spki_rejects_wrong_algorithm_oid() {
    // rsaEncryption (1.2.840.113549.1.1.1) with a stub BIT STRING.
    let der = hex_decode(
        "30 1a 30 0d 06 09 2a 86 48 86 f7 0d 01 01 01 05 00 \
         03 09 00 04 01 02 03 04 05 06 07",
    );
    assert!(EcdsaPublicKey::from_spki(&der).is_err());
}
