//! ECDSA signature verification over the P-256 curve, SHA-256 digest.
//!
//! Only verification; no key generation or signing. Not constant-time.

#![allow(dead_code)]

use crate::Error;
use crate::core::net::tls::p256::Point;

pub struct EcdsaPublicKey {
    pub point: Point,
}

/// OID 1.2.840.10045.2.1 id-ecPublicKey.
const OID_EC_PUBLIC_KEY: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01];
/// OID 1.2.840.10045.3.1.7 prime256v1 / secp256r1.
const OID_SECP256R1: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07];

impl EcdsaPublicKey {
    /// Parse SubjectPublicKeyInfo DER for an EC public key on P-256.
    pub fn from_spki(der: &[u8]) -> crate::Result<Self> {
        use crate::core::asn1::Parser;
        let mut top = Parser::new(der);
        let mut spki = top
            .read_sequence()
            .map_err(|e| Error::Tls(format!("ECDSA SPKI: {e}")))?;

        let mut alg = spki
            .read_sequence()
            .map_err(|e| Error::Tls(format!("ECDSA alg: {e}")))?;
        let oid = alg
            .read_oid()
            .map_err(|e| Error::Tls(format!("ECDSA alg OID: {e}")))?;
        if oid != OID_EC_PUBLIC_KEY {
            return Err(Error::Tls(format!(
                "ECDSA SPKI: expected id-ecPublicKey, got {}-byte OID",
                oid.len()
            )));
        }
        let curve_oid = alg
            .read_oid()
            .map_err(|e| Error::Tls(format!("ECDSA curve OID: {e}")))?;
        if curve_oid != OID_SECP256R1 {
            return Err(Error::Tls(format!(
                "ECDSA SPKI: expected secp256r1, got {}-byte OID",
                curve_oid.len()
            )));
        }
        while !alg.is_empty() {
            let _ = alg
                .read_any()
                .map_err(|e| Error::Tls(format!("ECDSA alg extra: {e}")))?;
        }

        let bits = spki
            .read_bit_string()
            .map_err(|e| Error::Tls(format!("ECDSA bit string: {e}")))?;
        if bits.unused_bits != 0 {
            return Err(Error::Tls(format!(
                "ECDSA SPKI: BIT STRING unused_bits = {}",
                bits.unused_bits
            )));
        }
        if bits.bytes.len() != 65 {
            return Err(Error::Tls(format!(
                "ECDSA SPKI: expected 65-byte point, got {}",
                bits.bytes.len()
            )));
        }
        let mut point_bytes = [0u8; 65];
        point_bytes.copy_from_slice(bits.bytes);
        let point = Point::from_uncompressed(&point_bytes)?;
        Ok(EcdsaPublicKey { point })
    }
}
