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

/// Cursor-style DER parser over a borrowed byte slice.
pub struct Parser<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Parser { data, pos: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.pos >= self.data.len()
    }

    pub fn remaining(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }

    /// Read a DER length encoding. Advances `pos`.
    /// Rejects indefinite length (0x80) and lengths needing >4 follow-up bytes.
    fn read_length(&mut self) -> crate::Result<usize> {
        let first = *self
            .data
            .get(self.pos)
            .ok_or_else(|| Error::Asn1("length: unexpected end of input".to_string()))?;
        self.pos += 1;

        if first < 0x80 {
            return Ok(first as usize);
        }
        if first == 0x80 {
            return Err(Error::Asn1(
                "length: indefinite form (BER) not allowed in DER".to_string(),
            ));
        }
        let n = (first & 0x7f) as usize;
        if n > 4 {
            return Err(Error::Asn1(format!(
                "length: long form with {n} follow-up bytes exceeds 4"
            )));
        }
        let mut len: usize = 0;
        for _ in 0..n {
            let b = *self
                .data
                .get(self.pos)
                .ok_or_else(|| Error::Asn1("length: truncated long form".to_string()))?;
            self.pos += 1;
            len = (len << 8) | (b as usize);
        }
        Ok(len)
    }

    /// Test-only accessor for `read_length`.
    #[doc(hidden)]
    pub fn read_length_for_test(&mut self) -> crate::Result<usize> {
        self.read_length()
    }

    /// Read one TLV, returning (tag, value bytes). Advances past the value.
    pub fn read_any(&mut self) -> crate::Result<(Tag, &'a [u8])> {
        let (tag, tag_bytes) = Tag::from_bytes(&self.data[self.pos..])?;
        self.pos += tag_bytes;

        let len = self.read_length()?;

        let end = self
            .pos
            .checked_add(len)
            .ok_or_else(|| Error::Asn1("value: length overflow".to_string()))?;
        if end > self.data.len() {
            return Err(Error::Asn1(format!(
                "value: truncated, need {} bytes, have {}",
                len,
                self.data.len() - self.pos
            )));
        }
        let value = &self.data[self.pos..end];
        self.pos = end;
        Ok((tag, value))
    }

    /// Peek at the next tag without advancing.
    pub fn peek_tag(&self) -> crate::Result<Tag> {
        let (tag, _consumed) = Tag::from_bytes(&self.data[self.pos..])?;
        Ok(tag)
    }

    /// Read one TLV and assert its tag matches `expected`. Returns the value.
    pub fn read_expect(&mut self, expected: Tag) -> crate::Result<&'a [u8]> {
        let (tag, value) = self.read_any()?;
        if tag != expected {
            return Err(Error::Asn1(format!(
                "expected tag {:?}, got {:?}",
                expected, tag
            )));
        }
        Ok(value)
    }

    /// Assert the parser has consumed all input.
    pub fn finish(self) -> crate::Result<()> {
        if self.pos != self.data.len() {
            return Err(Error::Asn1(format!(
                "finish: {} unconsumed bytes remain",
                self.data.len() - self.pos
            )));
        }
        Ok(())
    }
}
