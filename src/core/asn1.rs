//! ASN.1/DER parser for X.509, RSA, and ECDSA signature decoding.
//!
//! DER is a deterministic subset of BER. We do NOT support indefinite-length
//! encoding (0x80). The parser is zero-copy: all values are returned as
//! borrowed `&'a [u8]` slices into the input buffer.
//!
//! Not constant-time. Failure returns `Error::Asn1(String)`; no panic paths.

use crate::Error;

/// Tag class (upper 2 bits of the tag octet).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagClass {
    Universal,
    Application,
    ContextSpecific,
    Private,
}

impl TagClass {
    fn from_bits(bits: u8) -> Self {
        match bits & 0b11 {
            0b00 => TagClass::Universal,
            0b01 => TagClass::Application,
            0b10 => TagClass::ContextSpecific,
            _ => TagClass::Private,
        }
    }

    fn to_bits(self) -> u8 {
        match self {
            TagClass::Universal => 0b00,
            TagClass::Application => 0b01,
            TagClass::ContextSpecific => 0b10,
            TagClass::Private => 0b11,
        }
    }
}

/// Semantic decomposition of an ASN.1 tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tag {
    pub class: TagClass,
    pub constructed: bool,
    pub number: u32,
}

impl Tag {
    // Universal primitive
    pub const BOOLEAN: Tag = Tag {
        class: TagClass::Universal,
        constructed: false,
        number: 1,
    };
    pub const INTEGER: Tag = Tag {
        class: TagClass::Universal,
        constructed: false,
        number: 2,
    };
    pub const BIT_STRING: Tag = Tag {
        class: TagClass::Universal,
        constructed: false,
        number: 3,
    };
    pub const OCTET_STRING: Tag = Tag {
        class: TagClass::Universal,
        constructed: false,
        number: 4,
    };
    pub const NULL: Tag = Tag {
        class: TagClass::Universal,
        constructed: false,
        number: 5,
    };
    pub const OID: Tag = Tag {
        class: TagClass::Universal,
        constructed: false,
        number: 6,
    };
    pub const UTF8_STRING: Tag = Tag {
        class: TagClass::Universal,
        constructed: false,
        number: 12,
    };
    pub const PRINTABLE_STRING: Tag = Tag {
        class: TagClass::Universal,
        constructed: false,
        number: 19,
    };
    pub const IA5_STRING: Tag = Tag {
        class: TagClass::Universal,
        constructed: false,
        number: 22,
    };
    pub const UTC_TIME: Tag = Tag {
        class: TagClass::Universal,
        constructed: false,
        number: 23,
    };
    pub const GENERALIZED_TIME: Tag = Tag {
        class: TagClass::Universal,
        constructed: false,
        number: 24,
    };
    pub const BMP_STRING: Tag = Tag {
        class: TagClass::Universal,
        constructed: false,
        number: 30,
    };

    // Universal constructed
    pub const SEQUENCE: Tag = Tag {
        class: TagClass::Universal,
        constructed: true,
        number: 16,
    };
    pub const SET: Tag = Tag {
        class: TagClass::Universal,
        constructed: true,
        number: 17,
    };

    /// Context-specific tag builder.
    pub const fn context(number: u32, constructed: bool) -> Tag {
        Tag {
            class: TagClass::ContextSpecific,
            constructed,
            number,
        }
    }

    /// Decode a tag from the start of `input`. Returns (tag, bytes consumed).
    pub fn from_bytes(input: &[u8]) -> crate::Result<(Tag, usize)> {
        let first = *input
            .first()
            .ok_or_else(|| Error::Asn1("tag: empty input".to_string()))?;
        let class = TagClass::from_bits(first >> 6);
        let constructed = (first & 0b0010_0000) != 0;
        let low5 = first & 0b0001_1111;

        if low5 < 31 {
            return Ok((
                Tag {
                    class,
                    constructed,
                    number: low5 as u32,
                },
                1,
            ));
        }

        // High-tag-number form: accumulate 7 bits per byte until a byte with
        // high bit clear.
        let mut number: u32 = 0;
        let mut i = 1;
        loop {
            let b = *input
                .get(i)
                .ok_or_else(|| Error::Asn1("tag: truncated high-tag-number".to_string()))?;
            if number > (u32::MAX >> 7) {
                return Err(Error::Asn1("tag: number overflow".to_string()));
            }
            number = (number << 7) | ((b & 0x7f) as u32);
            i += 1;
            if b & 0x80 == 0 {
                break;
            }
        }
        Ok((
            Tag {
                class,
                constructed,
                number,
            },
            i,
        ))
    }

    /// Encode this tag as a single short-form byte. Returns `None` if the
    /// tag number is ≥ 31 (high-tag-number form required).
    pub fn to_short_byte(&self) -> Option<u8> {
        if self.number >= 31 {
            return None;
        }
        let mut b = self.class.to_bits() << 6;
        if self.constructed {
            b |= 0b0010_0000;
        }
        b |= self.number as u8 & 0x1f;
        Some(b)
    }
}
