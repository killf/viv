//! RSA PKCS#1 v1.5 signature verification (SHA-256 only).
//!
//! Verifies signatures produced by the private half of an RSA key pair.
//! Only public-key operations — not constant-time, not suitable for
//! private-key operations.

#![allow(dead_code)]

use crate::Error;
use crate::core::bigint::BigUint;

/// RSA public key: modulus `n` and exponent `e`.
pub struct RsaPublicKey {
    pub n: BigUint,
    pub e: BigUint,
}

impl RsaPublicKey {
    /// Construct from raw big-endian bytes.
    pub fn from_n_e(n_be: &[u8], e_be: &[u8]) -> Self {
        RsaPublicKey {
            n: BigUint::from_bytes_be(n_be),
            e: BigUint::from_bytes_be(e_be),
        }
    }

    /// Modulus byte length. Signatures must match this.
    pub fn n_byte_len(&self) -> usize {
        self.n.bit_len().div_ceil(8)
    }

    /// Parse a SubjectPublicKeyInfo DER-encoded byte string.
    ///
    /// ```text
    /// SubjectPublicKeyInfo ::= SEQUENCE {
    ///     algorithm AlgorithmIdentifier,
    ///     subjectPublicKey BIT STRING
    /// }
    /// RSAPublicKey ::= SEQUENCE { modulus INTEGER, publicExponent INTEGER }
    /// ```
    pub fn from_spki(der: &[u8]) -> crate::Result<Self> {
        use crate::core::asn1::Parser;

        /// OID 1.2.840.113549.1.1.1 rsaEncryption.
        const OID_RSA_ENCRYPTION: [u8; 9] =
            [0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01];

        let mut top = Parser::new(der);
        let mut spki = top
            .read_sequence()
            .map_err(|e| Error::Tls(format!("SPKI: {e}")))?;

        // AlgorithmIdentifier ::= SEQUENCE { algorithm OID, parameters ANY OPTIONAL }
        let mut alg = spki
            .read_sequence()
            .map_err(|e| Error::Tls(format!("SPKI algorithm: {e}")))?;
        let oid = alg
            .read_oid()
            .map_err(|e| Error::Tls(format!("SPKI OID: {e}")))?;
        if oid != OID_RSA_ENCRYPTION {
            return Err(Error::Tls(format!(
                "SPKI: non-RSA algorithm OID ({} bytes)",
                oid.len()
            )));
        }
        while !alg.is_empty() {
            let _ = alg
                .read_any()
                .map_err(|e| Error::Tls(format!("SPKI params: {e}")))?;
        }

        // subjectPublicKey BIT STRING wraps RSAPublicKey DER.
        let bits = spki
            .read_bit_string()
            .map_err(|e| Error::Tls(format!("SPKI bit string: {e}")))?;
        if bits.unused_bits != 0 {
            return Err(Error::Tls(format!(
                "SPKI bit string: expected 0 unused bits, got {}",
                bits.unused_bits
            )));
        }

        let mut rsa_pub = Parser::new(bits.bytes);
        let mut rsa_seq = rsa_pub
            .read_sequence()
            .map_err(|e| Error::Tls(format!("RSAPublicKey seq: {e}")))?;
        let n_bytes = rsa_seq
            .read_integer()
            .map_err(|e| Error::Tls(format!("RSAPublicKey n: {e}")))?;
        let e_bytes = rsa_seq
            .read_integer()
            .map_err(|e| Error::Tls(format!("RSAPublicKey e: {e}")))?;

        Ok(RsaPublicKey {
            n: BigUint::from_bytes_be(n_bytes),
            e: BigUint::from_bytes_be(e_bytes),
        })
    }
}

/// DER-encoded DigestInfo prefix for SHA-256. The 32-byte digest follows.
const SHA256_DIGEST_INFO_PREFIX: [u8; 19] = [
    0x30, 0x31, 0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03,
    0x04, 0x02, 0x01, 0x05, 0x00, 0x04, 0x20,
];

/// Verify an RSA PKCS#1 v1.5 signature over a SHA-256 digest.
///
/// Returns `Ok(())` on valid signature, `Err(Error::Tls(...))` with a
/// specific reason on any failure (length / range / EM[0] / EM[1] / PS /
/// terminator / prefix / digest).
pub fn verify_pkcs1_sha256_prehashed(
    pk: &RsaPublicKey,
    digest: &[u8; 32],
    signature: &[u8],
) -> crate::Result<()> {
    let k = pk.n_byte_len();

    if signature.len() != k {
        return Err(Error::Tls(format!(
            "RSA verify: signature length {} != modulus length {}",
            signature.len(),
            k
        )));
    }

    let s = BigUint::from_bytes_be(signature);
    if s.cmp(&pk.n) != std::cmp::Ordering::Less {
        return Err(Error::Tls("RSA verify: signature out of range".to_string()));
    }

    let m = s
        .modexp(&pk.e, &pk.n)
        .ok_or_else(|| Error::Tls("RSA verify: modexp failed (zero modulus)".to_string()))?;
    let em = m.to_bytes_be(k);

    if em.len() < 3 + 8 + SHA256_DIGEST_INFO_PREFIX.len() + 32 {
        return Err(Error::Tls(format!(
            "RSA verify: EM too short ({} bytes)",
            em.len()
        )));
    }
    if em[0] != 0x00 {
        return Err(Error::Tls(format!(
            "RSA verify: EM[0] = 0x{:02x}, expected 0x00",
            em[0]
        )));
    }
    if em[1] != 0x01 {
        return Err(Error::Tls(format!(
            "RSA verify: EM[1] = 0x{:02x}, expected 0x01 (block type)",
            em[1]
        )));
    }

    let mut i: usize = 2;
    while i < em.len() && em[i] == 0xff {
        i += 1;
    }
    let ps_len = i - 2;
    if ps_len < 8 {
        return Err(Error::Tls(format!(
            "RSA verify: PS length {ps_len} < 8"
        )));
    }
    if i >= em.len() {
        return Err(Error::Tls(
            "RSA verify: ran off end before PS terminator".to_string(),
        ));
    }
    if em[i] != 0x00 {
        return Err(Error::Tls(format!(
            "RSA verify: expected PS terminator 0x00, got 0x{:02x}",
            em[i]
        )));
    }
    i += 1;

    let t = &em[i..];
    let expected_t_len = SHA256_DIGEST_INFO_PREFIX.len() + 32;
    if t.len() != expected_t_len {
        return Err(Error::Tls(format!(
            "RSA verify: DigestInfo length {}, expected {}",
            t.len(),
            expected_t_len
        )));
    }
    if t[..SHA256_DIGEST_INFO_PREFIX.len()] != SHA256_DIGEST_INFO_PREFIX {
        return Err(Error::Tls(
            "RSA verify: DigestInfo prefix mismatch (wrong hash algorithm?)".to_string(),
        ));
    }
    if &t[SHA256_DIGEST_INFO_PREFIX.len()..] != digest.as_slice() {
        return Err(Error::Tls("RSA verify: digest mismatch".to_string()));
    }

    Ok(())
}
