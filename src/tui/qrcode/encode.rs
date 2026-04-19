use crate::Result;
use crate::error::Error;
use crate::tui::qrcode::rs::rs_encode;
use crate::tui::qrcode::tables::{BYTE_CAPACITY_M, EC_TABLE_M};

/// Interleaved data + ECC codewords and the QR version used.
pub struct EncodedData {
    pub data: Vec<u8>,
    pub version: u8,
}

/// Select the smallest QR version (EC level M) whose byte-mode capacity covers `byte_count`.
///
/// Returns `None` if `byte_count` is 0 or exceeds V40-M capacity.
pub fn select_version(byte_count: usize) -> Option<u8> {
    if byte_count == 0 {
        return None;
    }
    for (i, &cap) in BYTE_CAPACITY_M.iter().enumerate() {
        if byte_count <= cap as usize {
            return Some((i + 1) as u8);
        }
    }
    None
}

/// A simple bit accumulator.
struct BitStream {
    bytes: Vec<u8>,
    bit_pos: usize, // number of bits written so far
}

impl BitStream {
    fn new() -> Self {
        BitStream {
            bytes: Vec::new(),
            bit_pos: 0,
        }
    }

    /// Append the `count` least-significant bits of `value`.
    fn append(&mut self, value: u32, count: usize) {
        for i in (0..count).rev() {
            let bit = (value >> i) & 1;
            let byte_index = self.bit_pos / 8;
            let bit_index = 7 - (self.bit_pos % 8);
            if byte_index >= self.bytes.len() {
                self.bytes.push(0);
            }
            if bit == 1 {
                self.bytes[byte_index] |= 1 << bit_index;
            }
            self.bit_pos += 1;
        }
    }

    /// Number of bits written.
    fn len_bits(&self) -> usize {
        self.bit_pos
    }

    /// Consume the stream and return the underlying bytes.
    fn to_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Encode UTF-8 text in Byte mode with EC level M.
///
/// Returns `(data_codewords, version)` where `data_codewords` is padded to
/// `total_data` codewords for the chosen version.
pub fn encode_data(text: &str) -> Result<(Vec<u8>, u8)> {
    let bytes = text.as_bytes();
    let n = bytes.len();
    if n == 0 {
        return Err(Error::Qr("input must not be empty".to_string()));
    }

    let version = select_version(n)
        .ok_or_else(|| Error::Qr(format!("input too long ({} bytes) for QR code", n)))?;

    let (total_data, _ecc_per_block, _g1b, _g1d, _g2b, _g2d) = EC_TABLE_M[(version - 1) as usize];
    let total_data = total_data as usize;

    let mut bs = BitStream::new();

    // Mode indicator: Byte mode = 0b0100
    bs.append(0b0100, 4);

    // Character count indicator: 8 bits for V1-9, 16 bits for V10+
    if version <= 9 {
        bs.append(n as u32, 8);
    } else {
        bs.append(n as u32, 16);
    }

    // Data bytes
    for &b in bytes {
        bs.append(b as u32, 8);
    }

    // Terminator: up to 4 zero bits (but not beyond total_data * 8 bits)
    let current_bits = bs.len_bits();
    let capacity_bits = total_data * 8;
    let terminator_len = std::cmp::min(4, capacity_bits.saturating_sub(current_bits));
    if terminator_len > 0 {
        bs.append(0, terminator_len);
    }

    // Pad to byte boundary
    let remainder = bs.len_bits() % 8;
    if remainder != 0 {
        bs.append(0, 8 - remainder);
    }

    // Pad to total_data codewords with alternating 0xEC / 0x11
    let mut data = bs.to_bytes();
    let mut pad_byte = 0xEC_u8;
    while data.len() < total_data {
        data.push(pad_byte);
        pad_byte = if pad_byte == 0xEC { 0x11 } else { 0xEC };
    }

    Ok((data, version))
}

/// Encode `text`, split into RS blocks, compute ECC, and interleave.
///
/// The returned `EncodedData.data` contains all interleaved data codewords
/// followed by all interleaved ECC codewords.
pub fn encode_and_interleave(text: &str) -> Result<EncodedData> {
    let (data_codewords, version) = encode_data(text)?;
    let (_total_data, ecc_per_block, g1_blocks, g1_data, g2_blocks, g2_data) =
        EC_TABLE_M[(version - 1) as usize];

    let g1_blocks = g1_blocks as usize;
    let g1_data = g1_data as usize;
    let g2_blocks = g2_blocks as usize;
    let g2_data = g2_data as usize;
    let ecc_per_block = ecc_per_block as usize;

    // Split data codewords into blocks
    let mut data_blocks: Vec<Vec<u8>> = Vec::new();
    let mut offset = 0;

    for _ in 0..g1_blocks {
        data_blocks.push(data_codewords[offset..offset + g1_data].to_vec());
        offset += g1_data;
    }
    for _ in 0..g2_blocks {
        data_blocks.push(data_codewords[offset..offset + g2_data].to_vec());
        offset += g2_data;
    }

    // Compute ECC for each block
    let ecc_blocks: Vec<Vec<u8>> = data_blocks
        .iter()
        .map(|block| rs_encode(block, ecc_per_block))
        .collect();

    // Interleave data codewords column-wise
    let total_blocks = g1_blocks + g2_blocks;
    let max_data_len = if g2_blocks > 0 { g2_data } else { g1_data };
    let mut interleaved: Vec<u8> = Vec::new();

    for col in 0..max_data_len {
        for blk in 0..total_blocks {
            if col < data_blocks[blk].len() {
                interleaved.push(data_blocks[blk][col]);
            }
        }
    }

    // Interleave ECC codewords column-wise
    for col in 0..ecc_per_block {
        for blk in 0..total_blocks {
            interleaved.push(ecc_blocks[blk][col]);
        }
    }

    Ok(EncodedData {
        data: interleaved,
        version,
    })
}
