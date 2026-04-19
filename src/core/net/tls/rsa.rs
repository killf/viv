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
