//! ECDSA signature verification over the P-256 curve, SHA-256 digest.
//!
//! Only verification; no key generation or signing. Not constant-time.

#![allow(dead_code)]

use crate::core::net::tls::p256::Point;

pub struct EcdsaPublicKey {
    pub point: Point,
}
