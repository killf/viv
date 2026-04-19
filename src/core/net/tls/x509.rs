//! X.509 certificate parsing.
//!
//! Zero-copy DER decoder for X.509 v3 certificates. Exposes the minimal
//! field set needed by Phase 6 chain validation. All values borrow from
//! the input DER buffer.

#![allow(dead_code)]

// Error import lands with parsing methods in a subsequent commit.

/// X.509 certificate — all fields borrow from the input DER buffer.
pub struct X509Certificate<'a> {
    pub raw: &'a [u8],
    pub tbs_bytes: &'a [u8],
    pub version: u32,
    pub serial: &'a [u8],
    pub signature_algorithm: &'a [u8],
    pub issuer_dn: &'a [u8],
    pub subject_dn: &'a [u8],
    pub not_before: DateTime,
    pub not_after: DateTime,
    pub spki: &'a [u8],
    pub san_dns_names: Vec<&'a str>,
    pub is_ca: Option<bool>,
    pub signature: &'a [u8],
}

/// UTC date and time, resolution to seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}
