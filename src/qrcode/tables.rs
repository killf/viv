/// Error correction block info for EC level M, versions 1–40.
///
/// Each entry: `(total_data_codewords, ecc_per_block, g1_blocks, g1_data_per_block,
///               g2_blocks, g2_data_per_block)`
///
/// Source: ISO 18004 / https://www.thonky.com/qr-code-tutorial/error-correction-table
pub static EC_TABLE_M: [(u16, u8, u8, u8, u8, u8); 40] = [
    //  total  ecc  g1b  g1d  g2b  g2d
    (16, 10, 1, 16, 0, 0),      // V1
    (28, 16, 1, 28, 0, 0),      // V2
    (44, 26, 1, 44, 0, 0),      // V3
    (64, 18, 2, 32, 0, 0),      // V4
    (86, 24, 2, 43, 0, 0),      // V5
    (108, 16, 4, 27, 0, 0),     // V6
    (124, 18, 4, 31, 0, 0),     // V7
    (154, 22, 2, 38, 2, 39),    // V8
    (182, 22, 3, 36, 2, 37),    // V9
    (216, 26, 4, 43, 1, 44),    // V10
    (254, 30, 1, 50, 4, 51),    // V11
    (290, 22, 6, 36, 2, 37),    // V12
    (334, 22, 8, 37, 1, 38),    // V13
    (365, 24, 4, 40, 5, 41),    // V14
    (415, 24, 5, 41, 5, 42),    // V15
    (453, 28, 7, 45, 3, 46),    // V16
    (507, 28, 10, 46, 1, 47),   // V17
    (563, 26, 9, 43, 4, 44),    // V18
    (627, 26, 3, 44, 11, 45),   // V19
    (669, 26, 3, 41, 13, 42),   // V20
    (714, 26, 17, 42, 0, 0),    // V21
    (782, 28, 17, 46, 0, 0),    // V22
    (860, 28, 4, 47, 14, 48),   // V23
    (914, 28, 6, 45, 14, 46),   // V24
    (1000, 28, 8, 47, 13, 48),  // V25
    (1062, 28, 19, 46, 4, 47),  // V26
    (1128, 28, 22, 45, 3, 46),  // V27
    (1193, 28, 3, 45, 23, 46),  // V28
    (1267, 28, 21, 45, 7, 46),  // V29
    (1373, 28, 19, 47, 10, 48), // V30
    (1455, 28, 2, 46, 29, 47),  // V31
    (1541, 28, 10, 46, 23, 47), // V32
    (1631, 28, 14, 46, 21, 47), // V33
    (1725, 28, 14, 46, 23, 47), // V34
    (1812, 28, 12, 47, 26, 48), // V35
    (1914, 28, 6, 47, 34, 48),  // V36
    (1992, 28, 29, 46, 14, 47), // V37
    (2102, 28, 13, 46, 32, 47), // V38
    (2216, 28, 40, 47, 7, 48),  // V39
    (2334, 28, 18, 47, 31, 48), // V40
];

/// Maximum byte-mode character capacity for EC level M, versions 1–40.
///
/// Source: https://www.thonky.com/qr-code-tutorial/character-capacities
pub static BYTE_CAPACITY_M: [u16; 40] = [
    14, 26, 42, 62, 84, 106, 122, 152, 180, 213, 251, 287, 331, 362, 412, 450, 504, 560, 624, 666,
    711, 779, 857, 911, 997, 1059, 1125, 1190, 1264, 1370, 1452, 1538, 1628, 1722, 1809, 1911,
    1989, 2099, 2213, 2331,
];

/// Alignment pattern center coordinates for QR versions 1–40.
///
/// V1 has no alignment patterns. For each version, the positions list gives
/// all row/column coordinates at which an alignment pattern centre is placed.
///
/// Source: https://www.thonky.com/qr-code-tutorial/alignment-pattern-locations
pub static ALIGNMENT_POSITIONS: [&[u8]; 40] = [
    &[],                             // V1
    &[6, 18],                        // V2
    &[6, 22],                        // V3
    &[6, 26],                        // V4
    &[6, 30],                        // V5
    &[6, 34],                        // V6
    &[6, 22, 38],                    // V7
    &[6, 24, 42],                    // V8
    &[6, 26, 46],                    // V9
    &[6, 28, 50],                    // V10
    &[6, 30, 54],                    // V11
    &[6, 32, 58],                    // V12
    &[6, 34, 62],                    // V13
    &[6, 26, 46, 66],                // V14
    &[6, 26, 48, 70],                // V15
    &[6, 26, 50, 74],                // V16
    &[6, 30, 54, 78],                // V17
    &[6, 30, 56, 82],                // V18
    &[6, 30, 58, 86],                // V19
    &[6, 34, 62, 90],                // V20
    &[6, 28, 50, 72, 94],            // V21
    &[6, 26, 50, 74, 98],            // V22
    &[6, 30, 54, 78, 102],           // V23
    &[6, 28, 54, 80, 106],           // V24
    &[6, 32, 58, 84, 110],           // V25
    &[6, 30, 58, 86, 114],           // V26
    &[6, 34, 62, 90, 118],           // V27
    &[6, 26, 50, 74, 98, 122],       // V28
    &[6, 30, 54, 78, 102, 126],      // V29
    &[6, 26, 52, 78, 104, 130],      // V30
    &[6, 30, 56, 82, 108, 134],      // V31
    &[6, 34, 60, 86, 112, 138],      // V32
    &[6, 30, 58, 86, 114, 142],      // V33
    &[6, 34, 62, 90, 118, 146],      // V34
    &[6, 30, 54, 78, 102, 126, 150], // V35
    &[6, 24, 50, 76, 102, 128, 154], // V36
    &[6, 28, 54, 80, 106, 132, 158], // V37
    &[6, 32, 58, 84, 110, 136, 162], // V38
    &[6, 26, 54, 82, 110, 138, 166], // V39
    &[6, 30, 58, 86, 114, 142, 170], // V40
];

/// Pre-computed 15-bit format information strings for EC level M, mask patterns 0–7.
///
/// Format = BCH(15,5) of (ec_level_bits << 3 | mask) XOR 0x5412.
/// EC level M = 0b00.
///
/// Source: https://www.thonky.com/qr-code-tutorial/format-version-tables
pub static FORMAT_INFO_BITS_M: [u16; 8] = [
    0x5412, // mask 0: 101010000010010
    0x5125, // mask 1: 101000100100101
    0x5E7C, // mask 2: 101111001111100
    0x5B4B, // mask 3: 101101101001011
    0x45F9, // mask 4: 100010111111001
    0x40CE, // mask 5: 100000011001110
    0x4F97, // mask 6: 100111110010111
    0x4AA0, // mask 7: 100101010100000
];

/// Compute the 18-bit version information word for QR versions 7–40.
///
/// Uses BCH(18,6) with generator polynomial 0x1F25.
/// Returns `None` for versions below 7.
pub fn version_info(version: u8) -> Option<u32> {
    if version < 7 {
        return None;
    }
    let data = (version as u32) << 12;
    let mut rem = data;
    // Divide by generator 0x1F25 (degree 12) to get a 12-bit remainder
    for i in (0..6).rev() {
        if rem & (1 << (i + 12)) != 0 {
            rem ^= 0x1F25 << i;
        }
    }
    Some(data | rem)
}
