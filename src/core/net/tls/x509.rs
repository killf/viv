//! X.509 certificate parsing.
//!
//! Zero-copy DER decoder for X.509 v3 certificates. Exposes the minimal
//! field set needed by Phase 6 chain validation. All values borrow from
//! the input DER buffer.

#![allow(dead_code)]

use crate::Error;

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

impl DateTime {
    /// Parse 13-char UTCTime `YYMMDDHHMMSSZ`.
    /// Year mapping per RFC 5280 §4.1.2.5.1: YY < 50 → 20YY, YY ≥ 50 → 19YY.
    pub fn from_utc_time(s: &str) -> crate::Result<Self> {
        if s.len() != 13 {
            return Err(Error::Tls(format!(
                "UTCTime: expected 13 chars, got {}",
                s.len()
            )));
        }
        let b = s.as_bytes();
        if b[12] != b'Z' {
            return Err(Error::Tls("UTCTime: expected trailing 'Z'".to_string()));
        }
        let yy = two_digit(b, 0)?;
        let year: u16 = if yy < 50 {
            2000 + yy as u16
        } else {
            1900 + yy as u16
        };
        let month = two_digit(b, 2)?;
        let day = two_digit(b, 4)?;
        let hour = two_digit(b, 6)?;
        let minute = two_digit(b, 8)?;
        let second = two_digit(b, 10)?;
        validate_date_fields(month, day, hour, minute, second)?;
        Ok(DateTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
        })
    }

    /// Parse 15-char GeneralizedTime `YYYYMMDDHHMMSSZ`.
    pub fn from_generalized_time(s: &str) -> crate::Result<Self> {
        if s.len() != 15 {
            return Err(Error::Tls(format!(
                "GeneralizedTime: expected 15 chars, got {}",
                s.len()
            )));
        }
        let b = s.as_bytes();
        if b[14] != b'Z' {
            return Err(Error::Tls(
                "GeneralizedTime: expected trailing 'Z'".to_string(),
            ));
        }
        let hi = two_digit(b, 0)? as u16;
        let lo = two_digit(b, 2)? as u16;
        let year = hi * 100 + lo;
        let month = two_digit(b, 4)?;
        let day = two_digit(b, 6)?;
        let hour = two_digit(b, 8)?;
        let minute = two_digit(b, 10)?;
        let second = two_digit(b, 12)?;
        validate_date_fields(month, day, hour, minute, second)?;
        Ok(DateTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
        })
    }

    /// Current UTC time from `SystemTime::now`.
    pub fn now_utc() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let dur = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let total_secs = dur.as_secs() as i64;
        let days = total_secs / 86400;
        let remainder = total_secs % 86400;
        let hour = (remainder / 3600) as u8;
        let minute = ((remainder % 3600) / 60) as u8;
        let second = (remainder % 60) as u8;
        let (year, month, day) = rata_die(days);
        DateTime {
            year: year as u16,
            month,
            day,
            hour,
            minute,
            second,
        }
    }
}

/// Read 2 ASCII digits at `b[offset..offset+2]` as a decimal u8.
fn two_digit(b: &[u8], offset: usize) -> crate::Result<u8> {
    let h = b.get(offset).copied().unwrap_or(0);
    let l = b.get(offset + 1).copied().unwrap_or(0);
    if !h.is_ascii_digit() || !l.is_ascii_digit() {
        return Err(Error::Tls(format!(
            "time: non-digit at offset {offset}"
        )));
    }
    Ok((h - b'0') * 10 + (l - b'0'))
}

fn validate_date_fields(
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
) -> crate::Result<()> {
    if !(1..=12).contains(&month) {
        return Err(Error::Tls(format!("time: invalid month {month}")));
    }
    if !(1..=31).contains(&day) {
        return Err(Error::Tls(format!("time: invalid day {day}")));
    }
    if hour > 23 {
        return Err(Error::Tls(format!("time: invalid hour {hour}")));
    }
    if minute > 59 {
        return Err(Error::Tls(format!("time: invalid minute {minute}")));
    }
    if second > 60 {
        return Err(Error::Tls(format!("time: invalid second {second}")));
    }
    Ok(())
}

/// Days-since-1970 → (year, month, day). Howard Hinnant algorithm.
fn rata_die(days: i64) -> (i64, u8, u8) {
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = y + if m <= 2 { 1 } else { 0 };
    (y, m as u8, d as u8)
}
