use viv::core::net::tls::x509::{DateTime, X509Certificate};

const CERT_DER_HEX: &str = "\
308203433082022ba003020102021478ca833d92b4a4d8ccbf1fd88a168c31af555ab6300d06092a864886f70d01010b0500301b3119301706035504030c1074\
6573742e6578616d706c652e636f6d301e170d3236303431393031323131305a170d3336303431363031323131305a301b3119301706035504030c1074657374\
2e6578616d706c652e636f6d30820122300d06092a864886f70d01010105000382010f003082010a0282010100d574499a7b3114d73d1832c653c6d9c9cfddd2\
db93471e0bfb74b051b2c09777c9bbe7ee782b0ada6c4cc9eac1179c6e7e34a31fb04bff1d1f5254c90ca5926c84de12333e37dfc6330f481acb49734884ce29\
8bc51ae566cf6d1414922c92aa88fca6d686f70979747ee6cfb7ab1eb8a23f500e8d762d75d4ca895c5c520579ab67653c590e52f1d656571417f88821900e83\
d5f38ebe2d200965ff24176f1977677a1dcbf9cfe2ed1c4e0b3891f505fd8bdba7412386e523c537d188d70fe674e26cba0776788d8f5ed7e3a29d52484688df\
20e7b55ce5cee861af8296f08e966380653b612fbc919371f1a3f215a9f0d02ae5baf17aeb7133b0d2e93e7a350203010001a37f307d301d0603551d0e041604\
14736208bc0a9dae231eae60550a1b9db4fe76da73301f0603551d23041830168014736208bc0a9dae231eae60550a1b9db4fe76da73300f0603551d130101ff\
040530030101ff302a0603551d11042330218210746573742e6578616d706c652e636f6d820d2a2e6578616d706c652e636f6d300d06092a864886f70d01010b\
05000382010100c55a9206725b339e314528698c809de2fd28fd9647f024fafe59a1ab93ecd19c5826cda78586cb566e14c8ad297d5106e85bee2999808bd101\
543ea7ce2c24793ab62057900302fb784529996efc06ca7e4403163f3263fee1febb4550965200a7f4918ffb28b8f7dfafd22adae008f51485e9a1a27f070088\
1128e0f6e2615caf4964f1cd9a52121dc8183c3bbbe5b12a12cf67891c8d5c4d20c3337c4d3f8129fe118efaa9c79c712bd7e08f425db24747ac77a68e7ea77a\
affcd1d963eb9da2d01aa0b95a2225d1bfeca46f06fca89e3474611f97c49f949885eff4bcbcad9b40f54b10ec17ff10f00b59f86ad693bc8a7ce227e62e8978\
5fc70126270af6";

fn hex_decode(s: &str) -> Vec<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn cert_hex_decodes() {
    let der = hex_decode(CERT_DER_HEX);
    assert_eq!(der.len(), 839);
}

#[test]
fn datetime_ordering() {
    let earlier = DateTime {
        year: 2026,
        month: 4,
        day: 19,
        hour: 1,
        minute: 0,
        second: 0,
    };
    let later = DateTime {
        year: 2026,
        month: 4,
        day: 19,
        hour: 1,
        minute: 0,
        second: 1,
    };
    assert!(earlier < later);
}

#[test]
fn smoke_type_compiles() {
    let _: fn(&[u8]) = |_| {};
    // Ensure X509Certificate compiles with the expected fields.
    let _placeholder: Option<X509Certificate<'static>> = None;
}

#[test]
fn parse_utc_time_2020s() {
    let dt = DateTime::from_utc_time("260419012110Z").unwrap();
    assert_eq!(
        dt,
        DateTime {
            year: 2026,
            month: 4,
            day: 19,
            hour: 1,
            minute: 21,
            second: 10
        }
    );
}

#[test]
fn parse_utc_time_year_boundary_2049() {
    let dt = DateTime::from_utc_time("490101000000Z").unwrap();
    assert_eq!(dt.year, 2049);
}

#[test]
fn parse_utc_time_year_boundary_1950() {
    let dt = DateTime::from_utc_time("500101000000Z").unwrap();
    assert_eq!(dt.year, 1950);
}

#[test]
fn parse_utc_time_rejects_wrong_length() {
    assert!(DateTime::from_utc_time("26041901211").is_err());
    assert!(DateTime::from_utc_time("260419012110").is_err());
}

#[test]
fn parse_utc_time_rejects_non_z_suffix() {
    assert!(DateTime::from_utc_time("260419012110X").is_err());
}

#[test]
fn parse_utc_time_rejects_non_digit() {
    assert!(DateTime::from_utc_time("26041a012110Z").is_err());
}

#[test]
fn parse_utc_time_rejects_bad_month() {
    assert!(DateTime::from_utc_time("261319012110Z").is_err());
}

#[test]
fn parse_generalized_time_basic() {
    let dt = DateTime::from_generalized_time("20250102123456Z").unwrap();
    assert_eq!(
        dt,
        DateTime {
            year: 2025,
            month: 1,
            day: 2,
            hour: 12,
            minute: 34,
            second: 56
        }
    );
}

#[test]
fn parse_generalized_time_rejects_wrong_length() {
    assert!(DateTime::from_generalized_time("2025010212345Z").is_err());
}

#[test]
fn now_utc_has_plausible_year() {
    let dt = DateTime::now_utc();
    assert!(dt.year >= 2026 && dt.year < 2100);
    assert!((1..=12).contains(&dt.month));
    assert!((1..=31).contains(&dt.day));
}

#[test]
fn from_der_parses_version_and_serial() {
    let der = hex_decode(CERT_DER_HEX);
    let cert = X509Certificate::from_der(&der).unwrap();
    assert_eq!(cert.version, 2);
    assert_eq!(cert.serial.len(), 20);
    assert_eq!(cert.serial[0], 0x78);
}

#[test]
fn from_der_parses_validity() {
    let der = hex_decode(CERT_DER_HEX);
    let cert = X509Certificate::from_der(&der).unwrap();
    assert_eq!(
        cert.not_before,
        DateTime { year: 2026, month: 4, day: 19, hour: 1, minute: 21, second: 10 }
    );
    assert_eq!(
        cert.not_after,
        DateTime { year: 2036, month: 4, day: 16, hour: 1, minute: 21, second: 10 }
    );
}

#[test]
fn from_der_exposes_tbs_bytes() {
    let der = hex_decode(CERT_DER_HEX);
    let cert = X509Certificate::from_der(&der).unwrap();
    assert_eq!(cert.tbs_bytes[0], 0x30);
    assert!(cert.tbs_bytes.len() < cert.raw.len());
    assert!(!cert.tbs_bytes.is_empty());
}

#[test]
fn from_der_exposes_spki_and_phase3_parses_it() {
    let der = hex_decode(CERT_DER_HEX);
    let cert = X509Certificate::from_der(&der).unwrap();
    assert_eq!(cert.spki[0], 0x30);
    use viv::core::net::tls::rsa::RsaPublicKey;
    let pk = RsaPublicKey::from_spki(cert.spki).unwrap();
    assert_eq!(pk.n_byte_len(), 256);
}

#[test]
fn from_der_issuer_equals_subject_self_signed() {
    let der = hex_decode(CERT_DER_HEX);
    let cert = X509Certificate::from_der(&der).unwrap();
    assert_eq!(cert.issuer_dn, cert.subject_dn);
    assert_eq!(cert.subject_dn[0], 0x30);
}

#[test]
fn from_der_exposes_signature() {
    let der = hex_decode(CERT_DER_HEX);
    let cert = X509Certificate::from_der(&der).unwrap();
    assert_eq!(cert.signature.len(), 256);
}

#[test]
fn from_der_rejects_truncated() {
    assert!(X509Certificate::from_der(&[0x30]).is_err());
    assert!(X509Certificate::from_der(&[]).is_err());
}
