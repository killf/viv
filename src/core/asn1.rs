//! ASN.1/DER parser for X.509, RSA, and ECDSA signature decoding.
//!
//! DER is a deterministic subset of BER. We do NOT support indefinite-length
//! encoding (0x80). The parser is zero-copy: all values are returned as
//! borrowed `&'a [u8]` slices into the input buffer.
//!
//! Not constant-time. Failure returns `Error::Asn1(String)`; no panic paths.

#![allow(dead_code)]

/// Tag class (upper 2 bits of the tag octet).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagClass {
    Universal,
    Application,
    ContextSpecific,
    Private,
}

/// Semantic decomposition of an ASN.1 tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tag {
    pub class: TagClass,
    pub constructed: bool,
    pub number: u32,
}
