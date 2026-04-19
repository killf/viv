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

/// DER-encoded BIT STRING.
///
/// The first byte of the value in DER encoding is the number of unused bits
/// in the final byte (0-7). `bytes` is the payload after that prefix byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitString<'a> {
    pub unused_bits: u8,
    pub bytes: &'a [u8],
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

    /// Read a SEQUENCE and return a sub-parser over its contents.
    pub fn read_sequence(&mut self) -> crate::Result<Parser<'a>> {
        let value = self.read_expect(Tag::SEQUENCE)?;
        Ok(Parser::new(value))
    }

    /// Read a SET and return a sub-parser over its contents.
    pub fn read_set(&mut self) -> crate::Result<Parser<'a>> {
        let value = self.read_expect(Tag::SET)?;
        Ok(Parser::new(value))
    }

    /// Read a context-specific constructed [N] EXPLICIT wrapper and return a
    /// sub-parser over the wrapped value.
    pub fn read_explicit(&mut self, context_number: u32) -> crate::Result<Parser<'a>> {
        let expected = Tag::context(context_number, true);
        let value = self.read_expect(expected)?;
        Ok(Parser::new(value))
    }

    /// If the next TLV's tag matches `expected`, read and return its value.
    /// Otherwise, leave the position unchanged and return `None`.
    pub fn read_optional(&mut self, expected: Tag) -> crate::Result<Option<&'a [u8]>> {
        if self.is_empty() {
            return Ok(None);
        }
        let (tag, _consumed) = Tag::from_bytes(&self.data[self.pos..])?;
        if tag != expected {
            return Ok(None);
        }
        let value = self.read_expect(expected)?;
        Ok(Some(value))
    }

    /// Like `read_explicit` but returns `None` when the next tag does not
    /// match `[context_number] constructed`.
    pub fn read_optional_explicit(
        &mut self,
        context_number: u32,
    ) -> crate::Result<Option<Parser<'a>>> {
        if self.is_empty() {
            return Ok(None);
        }
        let (tag, _consumed) = Tag::from_bytes(&self.data[self.pos..])?;
        let expected = Tag::context(context_number, true);
        if tag != expected {
            return Ok(None);
        }
        let inner = self.read_explicit(context_number)?;
        Ok(Some(inner))
    }

    pub fn read_bool(&mut self) -> crate::Result<bool> {
        let value = self.read_expect(Tag::BOOLEAN)?;
        if value.len() != 1 {
            return Err(Error::Asn1(format!(
                "BOOLEAN: expected 1-byte value, got {}",
                value.len()
            )));
        }
        Ok(value[0] != 0)
    }

    pub fn read_integer(&mut self) -> crate::Result<&'a [u8]> {
        self.read_expect(Tag::INTEGER)
    }

    pub fn read_null(&mut self) -> crate::Result<()> {
        let value = self.read_expect(Tag::NULL)?;
        if !value.is_empty() {
            return Err(Error::Asn1(format!(
                "NULL: expected empty value, got {} bytes",
                value.len()
            )));
        }
        Ok(())
    }

    pub fn read_oid(&mut self) -> crate::Result<&'a [u8]> {
        self.read_expect(Tag::OID)
    }

    pub fn read_octet_string(&mut self) -> crate::Result<&'a [u8]> {
        self.read_expect(Tag::OCTET_STRING)
    }

    pub fn read_bit_string(&mut self) -> crate::Result<BitString<'a>> {
        let value = self.read_expect(Tag::BIT_STRING)?;
        if value.is_empty() {
            return Err(Error::Asn1(
                "BIT STRING: missing unused-bits prefix byte".to_string(),
            ));
        }
        let unused_bits = value[0];
        if unused_bits > 7 {
            return Err(Error::Asn1(format!(
                "BIT STRING: invalid unused-bits value {unused_bits} (>7)"
            )));
        }
        Ok(BitString {
            unused_bits,
            bytes: &value[1..],
        })
    }
}
