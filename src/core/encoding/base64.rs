use crate::error::Error;
use crate::Result;

const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn encode(input: &[u8]) -> String {
    let mut out = Vec::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((n >> 18) & 0x3f) as usize]);
        out.push(ALPHABET[((n >> 12) & 0x3f) as usize]);
        out.push(if chunk.len() > 1 {
            ALPHABET[((n >> 6) & 0x3f) as usize]
        } else {
            b'='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 0x3f) as usize]
        } else {
            b'='
        });
    }
    // SAFETY: all bytes pushed are either from ALPHABET (ASCII) or b'=' (ASCII).
    // The resulting slice is always valid UTF-8.
    unsafe { String::from_utf8_unchecked(out) }
}

pub fn decode(input: &str) -> Result<Vec<u8>> {
    let input = input.as_bytes();
    if input.len() % 4 != 0 {
        return Err(Error::Invariant("base64: invalid length".into()));
    }
    let mut table = [0xffu8; 256];
    for (i, &c) in ALPHABET.iter().enumerate() {
        table[c as usize] = i as u8;
    }
    table[b'=' as usize] = 0;
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    for chunk in input.chunks(4) {
        for &b in chunk.iter().take(2) {
            if b != b'=' && table[b as usize] == 0xff {
                return Err(Error::Invariant("base64: invalid character".into()));
            }
        }
        let n = ((table[chunk[0] as usize] as u32) << 18)
            | ((table[chunk[1] as usize] as u32) << 12)
            | ((table[chunk[2] as usize] as u32) << 6)
            | (table[chunk[3] as usize] as u32);
        out.push((n >> 16) as u8);
        if chunk[2] != b'=' {
            out.push((n >> 8) as u8);
        }
        if chunk[3] != b'=' {
            out.push(n as u8);
        }
    }
    Ok(out)
}
