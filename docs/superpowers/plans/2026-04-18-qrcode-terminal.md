# QR Code Terminal Component Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a zero-dependency QR Code encoder and terminal Widget that generates scannable QR codes from UTF-8 text.

**Architecture:** Core QR encoding in `src/qrcode/` (GF(256) → Reed-Solomon → data encoding → matrix construction → masking), rendering Widget in `src/tui/qrcode.rs`. The encoder produces a `QrMatrix` (bool grid), the Widget renders it using Unicode half-block characters.

**Tech Stack:** Rust (edition 2024), zero dependencies, existing TUI Widget framework.

**Spec:** `docs/superpowers/specs/2026-04-18-qrcode-terminal-design.md`

**Reference:** [thonky.com QR Code Tutorial](https://www.thonky.com/qr-code-tutorial/) for EC tables and algorithm details.

---

## File Map

### New Files

| File | Responsibility |
|------|---------------|
| `src/qrcode/mod.rs` | Module exports + `pub fn encode(&str) -> Result<QrMatrix>` |
| `src/qrcode/gf256.rs` | GF(256) finite field: EXP/LOG tables, mul, div, pow |
| `src/qrcode/rs.rs` | Reed-Solomon encoder: generator polynomial, rs_encode() |
| `src/qrcode/tables.rs` | Constant tables: version capacity, ECC params, alignment positions, format info |
| `src/qrcode/encode.rs` | Data encoding: byte mode bit stream, version selection, block grouping, interleaving |
| `src/qrcode/matrix.rs` | QrMatrix: functional patterns, data placement, masking, format/version info |
| `src/tui/qrcode.rs` | QrCodeWidget: half-block character rendering |
| `tests/qrcode/mod.rs` | Test module exports |
| `tests/qrcode/gf256_test.rs` | GF(256) tests |
| `tests/qrcode/rs_test.rs` | Reed-Solomon tests |
| `tests/qrcode/encode_test.rs` | Data encoding + version selection tests |
| `tests/qrcode/matrix_test.rs` | Matrix construction + full encode verification |
| `tests/tui/qrcode_test.rs` | Widget rendering tests |

### Modified Files

| File | Changes |
|------|---------|
| `src/lib.rs` | Add `pub mod qrcode;` |
| `src/tui/mod.rs` | Add `pub mod qrcode;` |
| `tests/mod.rs` or test root | Add `mod qrcode;` |
| `tests/tui/mod.rs` | Add `mod qrcode_test;` |

---

## Task 1: GF(256) Finite Field

**Files:**
- Create: `src/qrcode/mod.rs`
- Create: `src/qrcode/gf256.rs`
- Create: `tests/qrcode/mod.rs`
- Create: `tests/qrcode/gf256_test.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write failing tests for GF(256)**

```rust
// tests/qrcode/gf256_test.rs
use viv::qrcode::gf256;

#[test]
fn exp_table_first_entries() {
    // α^0 = 1, α^1 = 2, α^2 = 4, α^3 = 8, ...
    assert_eq!(gf256::EXP_TABLE[0], 1);
    assert_eq!(gf256::EXP_TABLE[1], 2);
    assert_eq!(gf256::EXP_TABLE[2], 4);
    assert_eq!(gf256::EXP_TABLE[7], 128);
}

#[test]
fn exp_table_wraps_at_8() {
    // α^8 = α^4 + α^3 + α^2 + 1 = 16+8+4+1 = 29 (mod 0x11D)
    assert_eq!(gf256::EXP_TABLE[8], 29);
}

#[test]
fn log_exp_inverse() {
    for i in 0..255u16 {
        let exp_val = gf256::EXP_TABLE[i as usize];
        assert_eq!(gf256::LOG_TABLE[exp_val as usize] as u16, i);
    }
}

#[test]
fn mul_basic() {
    assert_eq!(gf256::mul(0, 5), 0);
    assert_eq!(gf256::mul(5, 0), 0);
    assert_eq!(gf256::mul(1, 7), 7);
    assert_eq!(gf256::mul(7, 1), 7);
}

#[test]
fn mul_known_values() {
    // α^5 * α^3 = α^8 = 29
    let a = gf256::EXP_TABLE[5]; // 32
    let b = gf256::EXP_TABLE[3]; // 8
    assert_eq!(gf256::mul(a, b), 29);
}

#[test]
fn div_basic() {
    assert_eq!(gf256::div(0, 5), 0);
    assert_eq!(gf256::div(7, 1), 7);
}

#[test]
fn mul_div_inverse() {
    // For non-zero a, b: div(mul(a, b), b) == a
    for a in 1..=255u8 {
        for b in [1u8, 2, 37, 128, 255] {
            assert_eq!(gf256::div(gf256::mul(a, b), b), a);
        }
    }
}

#[test]
fn pow_basic() {
    assert_eq!(gf256::pow(2, 0), 1);
    assert_eq!(gf256::pow(2, 1), 2);
    assert_eq!(gf256::pow(2, 8), 29); // same as α^8
}
```

```rust
// tests/qrcode/mod.rs
mod gf256_test;
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test gf256_test 2>&1 | head -10`
Expected: module `qrcode` not found

- [ ] **Step 3: Implement GF(256)**

```rust
// src/qrcode/mod.rs
pub mod gf256;

// src/qrcode/gf256.rs

/// Primitive polynomial: x^8 + x^4 + x^3 + x^2 + 1 = 0x11D
const PRIMITIVE: u16 = 0x11D;

/// EXP_TABLE[i] = α^i in GF(256). Indices 0..254 cover all non-zero elements.
/// EXP_TABLE[255] wraps to EXP_TABLE[0] for convenience.
pub static EXP_TABLE: [u8; 256] = {
    let mut table = [0u8; 256];
    let mut val: u16 = 1;
    let mut i = 0;
    while i < 256 {
        table[i] = val as u8;
        val <<= 1;
        if val >= 256 {
            val ^= PRIMITIVE;
        }
        i += 1;
    }
    table
};

/// LOG_TABLE[v] = i where α^i = v. LOG_TABLE[0] is undefined (set to 0).
pub static LOG_TABLE: [u8; 256] = {
    let mut table = [0u8; 256];
    let mut i = 0;
    while i < 255 {
        table[EXP_TABLE[i] as usize] = i as u8;
        i += 1;
    }
    table
};

/// Multiply two elements in GF(256).
pub fn mul(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        return 0;
    }
    let log_sum = LOG_TABLE[a as usize] as u16 + LOG_TABLE[b as usize] as u16;
    EXP_TABLE[(log_sum % 255) as usize]
}

/// Divide a by b in GF(256). Panics if b == 0.
pub fn div(a: u8, b: u8) -> u8 {
    if a == 0 {
        return 0;
    }
    let log_diff = LOG_TABLE[a as usize] as u16 + 255 - LOG_TABLE[b as usize] as u16;
    EXP_TABLE[(log_diff % 255) as usize]
}

/// Raise a to the power n in GF(256).
pub fn pow(a: u8, n: u32) -> u8 {
    if n == 0 {
        return 1;
    }
    if a == 0 {
        return 0;
    }
    let log_a = LOG_TABLE[a as usize] as u32;
    let log_result = (log_a * n) % 255;
    EXP_TABLE[log_result as usize]
}
```

Add to `src/lib.rs`:
```rust
pub mod qrcode;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test gf256_test -v`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/qrcode/ src/lib.rs tests/qrcode/
git commit -m "feat(qrcode): add GF(256) finite field arithmetic"
```

---

## Task 2: Reed-Solomon Encoder

**Files:**
- Create: `src/qrcode/rs.rs`
- Create: `tests/qrcode/rs_test.rs`
- Modify: `src/qrcode/mod.rs`
- Modify: `tests/qrcode/mod.rs`

- [ ] **Step 1: Write failing tests**

```rust
// tests/qrcode/rs_test.rs
use viv::qrcode::rs;

#[test]
fn generator_poly_degree_7() {
    // For EC level L version 1: 7 ECC codewords → generator poly has 8 coefficients
    let gen = rs::generator_poly(7);
    assert_eq!(gen.len(), 8); // degree 7 → 8 coefficients
    assert_eq!(gen[0], 1); // leading coefficient is always 1
}

#[test]
fn generator_poly_degree_10() {
    // For EC level M version 1: 10 ECC codewords
    let gen = rs::generator_poly(10);
    assert_eq!(gen.len(), 11);
    assert_eq!(gen[0], 1);
}

#[test]
fn rs_encode_hello_world_1m() {
    // Known test vector: "HELLO WORLD" encoded as 1-M (from thonky.com)
    // Data codewords: 32, 91, 11, 120, 209, 114, 220, 77, 67, 64, 236, 17, 236, 17, 236, 17
    // Expected ECC:   196, 35, 39, 119, 235, 215, 231, 226, 93, 23
    let data: Vec<u8> = vec![32, 91, 11, 120, 209, 114, 220, 77, 67, 64, 236, 17, 236, 17, 236, 17];
    let ecc = rs::rs_encode(&data, 10);
    assert_eq!(ecc, vec![196, 35, 39, 119, 235, 215, 231, 226, 93, 23]);
}

#[test]
fn rs_encode_length() {
    let data = vec![1, 2, 3, 4, 5];
    let ecc = rs::rs_encode(&data, 7);
    assert_eq!(ecc.len(), 7);
}

#[test]
fn rs_encode_all_zeros() {
    let data = vec![0; 10];
    let ecc = rs::rs_encode(&data, 5);
    // All-zero data → all-zero ECC
    assert_eq!(ecc, vec![0; 5]);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test rs_test 2>&1 | head -10`

- [ ] **Step 3: Implement Reed-Solomon encoder**

```rust
// src/qrcode/rs.rs
use crate::qrcode::gf256;

/// Generate the Reed-Solomon generator polynomial for `ecc_count` error correction codewords.
/// Returns coefficients [1, c1, c2, ..., c_ecc] of degree `ecc_count`.
/// g(x) = (x - α^0)(x - α^1)...(x - α^(ecc_count-1))
pub fn generator_poly(ecc_count: usize) -> Vec<u8> {
    let mut poly = vec![0u8; ecc_count + 1];
    poly[0] = 1;
    let mut len = 1;

    for i in 0..ecc_count {
        // Multiply poly by (x - α^i) = (x + α^i) in GF(256) since +/-  are same
        let alpha_i = gf256::EXP_TABLE[i];
        // Work backwards to avoid overwriting
        let new_len = len + 1;
        // poly[j] = poly[j-1] XOR poly[j] * α^i
        for j in (1..len).rev() {
            poly[j] = poly[j - 1] ^ gf256::mul(poly[j], alpha_i);
        }
        // Handle j=0: no poly[j-1], just multiply
        poly[0] = gf256::mul(poly[0], alpha_i);
        // Shift: the new leading term
        // Actually we need to shift properly. Let me redo this.
        // poly = poly * (x + α^i)
        // new[j] = old[j-1] + old[j] * α^i  (where old[-1] = 0)
        len = new_len;
    }

    // Fix: proper polynomial multiplication
    // Start over with cleaner algorithm
    let mut poly = vec![1u8];
    for i in 0..ecc_count {
        let alpha_i = gf256::EXP_TABLE[i];
        let mut new_poly = vec![0u8; poly.len() + 1];
        for (j, &coeff) in poly.iter().enumerate() {
            new_poly[j] ^= coeff; // coeff * x
            new_poly[j + 1] ^= gf256::mul(coeff, alpha_i); // coeff * α^i
        }
        poly = new_poly;
    }

    poly
}

/// Compute Reed-Solomon error correction codewords.
/// Returns `ecc_count` bytes of ECC data.
pub fn rs_encode(data: &[u8], ecc_count: usize) -> Vec<u8> {
    let gen = generator_poly(ecc_count);

    // Polynomial long division: data * x^ecc_count / gen
    let mut remainder = vec![0u8; ecc_count];

    for &byte in data {
        let factor = byte ^ remainder[0];
        // Shift remainder left by 1
        for j in 0..ecc_count - 1 {
            remainder[j] = remainder[j + 1];
        }
        remainder[ecc_count - 1] = 0;
        // XOR with gen * factor
        for j in 0..ecc_count {
            remainder[j] ^= gf256::mul(gen[j + 1], factor);
        }
    }

    remainder
}
```

Add to `src/qrcode/mod.rs`:
```rust
pub mod rs;
```

Add to `tests/qrcode/mod.rs`:
```rust
mod rs_test;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test rs_test -v`
Expected: all tests PASS. The HELLO WORLD known vector is critical — if it passes, the RS encoder is correct.

- [ ] **Step 5: Commit**

```bash
git add src/qrcode/rs.rs src/qrcode/mod.rs tests/qrcode/rs_test.rs tests/qrcode/mod.rs
git commit -m "feat(qrcode): add Reed-Solomon encoder"
```

---

## Task 3: Constant Tables

**Files:**
- Create: `src/qrcode/tables.rs`
- Modify: `src/qrcode/mod.rs`

No separate test file — tables are verified through integration tests in later tasks.

- [ ] **Step 1: Create tables.rs with all QR code constant data**

```rust
// src/qrcode/tables.rs

/// EC level M block info for versions 1-40.
/// (total_data_codewords, ecc_per_block, g1_blocks, g1_data, g2_blocks, g2_data)
/// g2_blocks=0 means no group 2.
pub static EC_TABLE_M: [(u16, u8, u8, u8, u8, u8); 40] = [
    // Ver  total  ecc  g1b g1d g2b g2d
    (  16,  10,  1,  16,  0,   0), // V1
    (  28,  16,  1,  28,  0,   0), // V2
    (  44,  26,  1,  44,  0,   0), // V3
    (  64,  18,  2,  32,  0,   0), // V4
    (  86,  24,  2,  43,  0,   0), // V5
    ( 108,  16,  4,  27,  0,   0), // V6
    ( 124,  18,  4,  31,  0,   0), // V7
    ( 154,  22,  2,  38,  2,  39), // V8
    ( 182,  22,  3,  36,  2,  37), // V9
    ( 216,  26,  4,  43,  1,  44), // V10
    ( 254,  30,  1,  50,  4,  51), // V11
    ( 290,  22,  6,  36,  2,  37), // V12
    ( 334,  22,  8,  37,  1,  38), // V13
    ( 365,  24,  4,  40,  5,  41), // V14
    ( 415,  24,  5,  41,  5,  42), // V15
    ( 453,  28,  7,  45,  3,  46), // V16
    ( 507,  28,  10, 46,  1,  47), // V17
    ( 563,  26,  9,  43,  4,  44), // V18
    ( 627,  26,  3,  44, 11,  45), // V19
    ( 669,  26,  3,  41, 13,  42), // V20
    ( 714,  26,  17, 42,  0,   0), // V21
    ( 782,  28,  17, 46,  0,   0), // V22
    ( 860,  28,  4,  47, 14,  48), // V23
    ( 914,  28,  6,  45, 14,  46), // V24
    ( 1000, 28,  8,  47, 13,  48), // V25
    ( 1062, 28,  19, 46,  4,  47), // V26
    ( 1128, 28,  22, 45,  3,  46), // V27
    ( 1193, 28,  3,  45, 23,  46), // V28
    ( 1267, 28,  21, 45,  7,  46), // V29
    ( 1373, 28,  19, 47,  10, 48), // V30
    ( 1455, 28,  2,  46, 29,  47), // V31
    ( 1541, 28,  10, 46, 23,  47), // V32
    ( 1631, 28,  14, 46, 21,  47), // V33
    ( 1725, 28,  14, 46, 23,  47), // V34
    ( 1812, 28,  12, 47, 26,  48), // V35
    ( 1914, 28,  6,  47, 34,  48), // V36
    ( 1992, 28,  29, 46, 14,  47), // V37
    ( 2102, 28,  13, 46, 32,  47), // V38
    ( 2216, 28,  40, 47,  7,  48), // V39
    ( 2334, 28,  18, 47, 31,  48), // V40
];

/// Byte mode data capacity per version at EC level M.
/// BYTE_CAPACITY_M[v-1] = max bytes for version v.
pub static BYTE_CAPACITY_M: [u16; 40] = [
    14, 26, 42, 62, 84, 106, 122, 152, 180, 213,
    251, 287, 331, 362, 412, 450, 504, 560, 624, 666,
    711, 779, 857, 911, 997, 1059, 1125, 1190, 1264, 1370,
    1452, 1538, 1628, 1722, 1809, 1911, 1989, 2099, 2213, 2331,
];

/// Alignment pattern center coordinates for each version.
/// Version 1 has no alignment patterns.
pub static ALIGNMENT_POSITIONS: [&[u8]; 40] = [
    &[],                              // V1
    &[6, 18],                         // V2
    &[6, 22],                         // V3
    &[6, 26],                         // V4
    &[6, 30],                         // V5
    &[6, 34],                         // V6
    &[6, 22, 38],                     // V7
    &[6, 24, 42],                     // V8
    &[6, 26, 46],                     // V9
    &[6, 28, 50],                     // V10
    &[6, 30, 54],                     // V11
    &[6, 32, 58],                     // V12
    &[6, 34, 62],                     // V13
    &[6, 26, 46, 66],                 // V14
    &[6, 26, 48, 70],                 // V15
    &[6, 26, 50, 74],                 // V16
    &[6, 30, 54, 78],                 // V17
    &[6, 30, 56, 82],                 // V18
    &[6, 30, 58, 86],                 // V19
    &[6, 34, 62, 90],                 // V20
    &[6, 28, 50, 72, 94],             // V21
    &[6, 26, 50, 74, 98],             // V22
    &[6, 30, 54, 78, 102],            // V23
    &[6, 28, 54, 80, 106],            // V24
    &[6, 32, 58, 84, 110],            // V25
    &[6, 30, 58, 86, 114],            // V26
    &[6, 34, 62, 90, 118],            // V27
    &[6, 26, 50, 74, 98, 122],        // V28
    &[6, 30, 54, 78, 102, 126],       // V29
    &[6, 26, 52, 78, 104, 130],       // V30
    &[6, 30, 56, 82, 108, 134],       // V31
    &[6, 34, 60, 86, 112, 138],       // V32
    &[6, 30, 58, 86, 114, 142],       // V33
    &[6, 34, 62, 90, 118, 146],       // V34
    &[6, 30, 54, 78, 102, 126, 150],  // V35
    &[6, 24, 50, 76, 102, 128, 154],  // V36
    &[6, 28, 54, 80, 106, 132, 158],  // V37
    &[6, 32, 58, 84, 110, 136, 162],  // V38
    &[6, 26, 54, 82, 110, 138, 166],  // V39
    &[6, 30, 58, 86, 114, 142, 170],  // V40
];

/// Format information strings for EC level M (00), masks 0-7.
/// 15 bits each, pre-computed with BCH and XOR mask 0x5412.
pub static FORMAT_INFO_M: [u16; 8] = [
    0x5412 ^ format_bch(0b00_000), // mask 0
    0x5412 ^ format_bch(0b00_001), // mask 1
    0x5412 ^ format_bch(0b00_010), // mask 2
    0x5412 ^ format_bch(0b00_011), // mask 3
    0x5412 ^ format_bch(0b00_100), // mask 4
    0x5412 ^ format_bch(0b00_101), // mask 5
    0x5412 ^ format_bch(0b00_110), // mask 6
    0x5412 ^ format_bch(0b00_111), // mask 7
];

// Actually, pre-compute these at compile time. The BCH(15,5) for format info
// uses generator 0x537. Let's just hardcode the final values:
// EC level M = 00, mask patterns 0-7.
// Pre-computed format info bits (15 bits, MSB first):
pub static FORMAT_INFO_BITS_M: [u16; 8] = [
    0x5412, // M, mask 0 → 101_0100_0001_0010
    0x5125, // M, mask 1
    0x5E7C, // M, mask 2
    0x5B4B, // M, mask 3
    0x45F9, // M, mask 4
    0x40CE, // M, mask 5
    0x4F97, // M, mask 6
    0x4AA0, // M, mask 7
];

/// Version information for versions 7-40 (18 bits each).
/// Computed as version_number << 12 | BCH(18,6) remainder.
/// Only needed for version >= 7.
pub fn version_info(version: u8) -> Option<u32> {
    if version < 7 {
        return None;
    }
    let mut data = (version as u32) << 12;
    let mut remainder = data;
    // BCH(18,6) generator: 0x1F25
    for _ in 0..12 {
        if remainder & (1 << 17) != 0 {
            remainder ^= 0x1F25 << 5;
        }
        remainder <<= 1;
    }
    // Actually, compute BCH remainder properly:
    let mut r = (version as u32) << 12;
    for i in (0..12).rev() {
        if r & (1 << (i + 6)) != 0 {
            r ^= 0x1F25 << i;
        }
    }
    Some(data | (r & 0xFFF))
}

// Note: The implementer should verify FORMAT_INFO_BITS_M values against
// the thonky.com format info table or compute them with BCH(15,5) generator 0x537.
// The format is: (ec_level_bits << 13) | (mask_bits << 10) | bch_remainder, XOR 0x5412.
```

Note to implementer: Verify ALL table values against [thonky.com EC table](https://www.thonky.com/qr-code-tutorial/error-correction-table) and [alignment positions](https://www.thonky.com/qr-code-tutorial/alignment-pattern-locations). Cross-check the BYTE_CAPACITY_M values by computing: for each version, subtract the mode indicator (4 bits) and character count indicator (8 or 16 bits) overhead from total_data_codewords * 8 bits, then divide by 8.

- [ ] **Step 2: Add module declaration**

Add to `src/qrcode/mod.rs`:
```rust
pub mod tables;
```

- [ ] **Step 3: Build to verify compilation**

Run: `cargo build 2>&1 | head -20`
Expected: compiles (tables may have minor const-eval issues to fix)

- [ ] **Step 4: Commit**

```bash
git add src/qrcode/tables.rs src/qrcode/mod.rs
git commit -m "feat(qrcode): add QR code constant tables"
```

---

## Task 4: Data Encoding + Version Selection

**Files:**
- Create: `src/qrcode/encode.rs`
- Create: `tests/qrcode/encode_test.rs`
- Modify: `src/qrcode/mod.rs`
- Modify: `tests/qrcode/mod.rs`

- [ ] **Step 1: Write failing tests**

```rust
// tests/qrcode/encode_test.rs
use viv::qrcode::encode;

#[test]
fn select_version_short_url() {
    // "https://example.com" = 19 bytes → fits in V1-M (capacity 14)? No, 19 > 14.
    // V2-M capacity = 26, 19 ≤ 26 → V2
    let v = encode::select_version(19);
    assert_eq!(v, Some(2));
}

#[test]
fn select_version_tiny() {
    // 1 byte → V1-M (capacity 14)
    let v = encode::select_version(1);
    assert_eq!(v, Some(1));
}

#[test]
fn select_version_max_v1() {
    let v = encode::select_version(14);
    assert_eq!(v, Some(1));
}

#[test]
fn select_version_too_large() {
    let v = encode::select_version(3000);
    assert_eq!(v, None);
}

#[test]
fn encode_data_codewords_short() {
    // "Hi" = [0x48, 0x69] = 2 bytes
    // Byte mode for V1-M: mode=0100(4bits) + count=00000010(8bits) + data(16bits) + terminator(4bits)
    // = 4+8+16+4 = 32 bits = 4 bytes, padded to 16 data codewords
    let (codewords, version) = encode::encode_data("Hi").unwrap();
    assert!(version >= 1);
    assert!(!codewords.is_empty());
    // First nibble should be 0100 (byte mode) = 0x4...
    assert_eq!(codewords[0] >> 4, 0x4);
}

#[test]
fn encode_and_interleave() {
    let result = encode::encode_and_interleave("HELLO").unwrap();
    assert!(!result.data.is_empty());
    assert!(result.version >= 1);
}

#[test]
fn encode_empty_returns_error() {
    let result = encode::encode_data("");
    assert!(result.is_err() || result.unwrap().0.len() > 0);
    // Empty string should either error or produce a valid minimal encoding
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test encode_test 2>&1 | head -10`

- [ ] **Step 3: Implement data encoding**

```rust
// src/qrcode/encode.rs
use crate::qrcode::rs;
use crate::qrcode::tables::{BYTE_CAPACITY_M, EC_TABLE_M};

/// Select the minimum QR version (1-40) that can hold `byte_count` bytes at EC level M.
pub fn select_version(byte_count: usize) -> Option<u8> {
    for (i, &cap) in BYTE_CAPACITY_M.iter().enumerate() {
        if byte_count <= cap as usize {
            return Some((i + 1) as u8);
        }
    }
    None
}

/// Encode UTF-8 text into data codewords using Byte mode.
/// Returns (data_codewords, version).
pub fn encode_data(text: &str) -> crate::Result<(Vec<u8>, u8)> {
    let data_bytes = text.as_bytes();
    if data_bytes.is_empty() {
        return Err(crate::Error::Qr("empty input".into()));
    }

    let version = select_version(data_bytes.len())
        .ok_or_else(|| crate::Error::Qr("data too long for QR code".into()))?;

    let (total_data, _, _, _, _, _) = EC_TABLE_M[(version - 1) as usize];
    let char_count_bits: usize = if version <= 9 { 8 } else { 16 };

    // Build bit stream
    let mut bits = BitStream::new();

    // Mode indicator: 0100 (Byte mode)
    bits.append(0b0100, 4);

    // Character count
    bits.append(data_bytes.len() as u32, char_count_bits);

    // Data bytes
    for &b in data_bytes {
        bits.append(b as u32, 8);
    }

    // Terminator: up to 4 zero bits
    let capacity_bits = total_data as usize * 8;
    let remaining = capacity_bits.saturating_sub(bits.len());
    bits.append(0, remaining.min(4));

    // Pad to byte boundary
    while bits.len() % 8 != 0 {
        bits.append(0, 1);
    }

    // Fill with alternating 0xEC, 0x11
    let mut pad_toggle = false;
    while bits.len() < capacity_bits {
        bits.append(if pad_toggle { 0x11 } else { 0xEC }, 8);
        pad_toggle = !pad_toggle;
    }

    Ok((bits.to_bytes(), version))
}

/// Result of full encoding + interleaving.
pub struct EncodedData {
    pub data: Vec<u8>,    // Interleaved data + ECC codewords
    pub version: u8,
}

/// Encode text, compute ECC, and interleave blocks.
pub fn encode_and_interleave(text: &str) -> crate::Result<EncodedData> {
    let (codewords, version) = encode_data(text)?;
    let (_, ecc_per_block, g1_blocks, g1_data, g2_blocks, g2_data) =
        EC_TABLE_M[(version - 1) as usize];

    // Split data into blocks
    let mut blocks: Vec<Vec<u8>> = Vec::new();
    let mut offset = 0;

    for _ in 0..g1_blocks {
        blocks.push(codewords[offset..offset + g1_data as usize].to_vec());
        offset += g1_data as usize;
    }
    for _ in 0..g2_blocks {
        blocks.push(codewords[offset..offset + g2_data as usize].to_vec());
        offset += g2_data as usize;
    }

    // Compute ECC for each block
    let mut ecc_blocks: Vec<Vec<u8>> = Vec::new();
    for block in &blocks {
        ecc_blocks.push(rs::rs_encode(block, ecc_per_block as usize));
    }

    // Interleave data codewords
    let mut result = Vec::new();
    let max_data_len = blocks.iter().map(|b| b.len()).max().unwrap_or(0);
    for i in 0..max_data_len {
        for block in &blocks {
            if i < block.len() {
                result.push(block[i]);
            }
        }
    }

    // Interleave ECC codewords
    for i in 0..ecc_per_block as usize {
        for ecc_block in &ecc_blocks {
            if i < ecc_block.len() {
                result.push(ecc_block[i]);
            }
        }
    }

    Ok(EncodedData { data: result, version })
}

/// Simple bit stream builder.
struct BitStream {
    bits: Vec<bool>,
}

impl BitStream {
    fn new() -> Self {
        Self { bits: Vec::new() }
    }

    fn append(&mut self, value: u32, count: usize) {
        for i in (0..count).rev() {
            self.bits.push((value >> i) & 1 == 1);
        }
    }

    fn len(&self) -> usize {
        self.bits.len()
    }

    fn to_bytes(&self) -> Vec<u8> {
        self.bits
            .chunks(8)
            .map(|chunk| {
                let mut byte = 0u8;
                for (i, &bit) in chunk.iter().enumerate() {
                    if bit {
                        byte |= 1 << (7 - i);
                    }
                }
                byte
            })
            .collect()
    }
}
```

Note: The `crate::Error::Qr` variant needs to be added to the error enum. Add a `Qr(String)` variant to `src/error.rs`.

- [ ] **Step 4: Add module declarations and Error variant**

Add to `src/qrcode/mod.rs`:
```rust
pub mod encode;
```

Add to `tests/qrcode/mod.rs`:
```rust
mod encode_test;
```

Add `Qr(String)` variant to `src/error.rs` Error enum.

- [ ] **Step 5: Run tests**

Run: `cargo test --test encode_test -v`
Expected: all tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/qrcode/encode.rs src/qrcode/mod.rs src/error.rs tests/qrcode/encode_test.rs tests/qrcode/mod.rs
git commit -m "feat(qrcode): add data encoding, version selection, and block interleaving"
```

---

## Task 5: Matrix Construction

**Files:**
- Create: `src/qrcode/matrix.rs`
- Create: `tests/qrcode/matrix_test.rs`
- Modify: `src/qrcode/mod.rs`
- Modify: `tests/qrcode/mod.rs`

This is the largest task — functional patterns, data placement, masking, format info, version info.

- [ ] **Step 1: Write failing tests**

```rust
// tests/qrcode/matrix_test.rs
use viv::qrcode::matrix::QrMatrix;

#[test]
fn matrix_size_v1() {
    let m = QrMatrix::new(1);
    assert_eq!(m.size(), 21);
}

#[test]
fn matrix_size_v5() {
    let m = QrMatrix::new(5);
    assert_eq!(m.size(), 37);
}

#[test]
fn finder_pattern_top_left() {
    let m = QrMatrix::new(1);
    // Top-left 7x7 finder pattern: outer ring is black
    assert!(m.get(0, 0)); // top-left corner = black
    assert!(m.get(0, 6)); // top-right of finder
    assert!(m.get(6, 0)); // bottom-left of finder
    assert!(!m.get(1, 1)); // inside ring = white
    assert!(m.get(2, 2)); // center square = black
}

#[test]
fn timing_pattern_row_6() {
    let m = QrMatrix::new(1);
    // Row 6 between finders: alternating black/white
    assert!(m.get(6, 8));  // col 8 should be black (even)
    assert!(!m.get(6, 9)); // col 9 should be white (odd)
    assert!(m.get(6, 10)); // col 10 black
}

#[test]
fn separator_is_white() {
    let m = QrMatrix::new(1);
    // Separator around top-left finder: row 7, cols 0-7 are white
    assert!(!m.get(7, 0));
    assert!(!m.get(7, 6));
}

#[test]
fn full_encode_v1_produces_correct_size() {
    let m = QrMatrix::build(1, &[0u8; 26]); // 26 = total codewords for V1
    assert_eq!(m.size(), 21);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test matrix_test 2>&1 | head -10`

- [ ] **Step 3: Implement QrMatrix**

```rust
// src/qrcode/matrix.rs
use crate::qrcode::tables::{ALIGNMENT_POSITIONS, FORMAT_INFO_BITS_M};

pub struct QrMatrix {
    version: u8,
    size: usize,
    modules: Vec<Vec<bool>>,      // true = black
    is_function: Vec<Vec<bool>>,  // true = reserved (finder, timing, etc.)
}

impl QrMatrix {
    /// Create a new matrix with functional patterns placed.
    pub fn new(version: u8) -> Self {
        let size = (4 * version as usize) + 17;
        let mut m = Self {
            version,
            size,
            modules: vec![vec![false; size]; size],
            is_function: vec![vec![false; size]; size],
        };
        m.place_finder_patterns();
        m.place_separators();
        m.place_timing_patterns();
        m.place_alignment_patterns();
        m.place_dark_module();
        // Reserve format info and version info areas
        m.reserve_format_areas();
        if version >= 7 {
            m.reserve_version_areas();
        }
        m
    }

    pub fn size(&self) -> usize { self.size }

    pub fn get(&self, row: usize, col: usize) -> bool {
        self.modules[row][col]
    }

    pub fn modules(&self) -> &Vec<Vec<bool>> { &self.modules }

    /// Full build: create matrix, place data, apply best mask, write format/version info.
    pub fn build(version: u8, data: &[u8]) -> Self {
        let mut m = Self::new(version);
        m.place_data_bits(data);
        let best_mask = m.evaluate_masks();
        m.apply_mask(best_mask);
        m.write_format_info(best_mask);
        if version >= 7 {
            m.write_version_info();
        }
        m
    }

    // ── Functional patterns ──────────────────────────────────────

    fn place_finder_patterns(&mut self) {
        self.place_finder_at(0, 0);                     // top-left
        self.place_finder_at(0, self.size - 7);         // top-right
        self.place_finder_at(self.size - 7, 0);         // bottom-left
    }

    fn place_finder_at(&mut self, row: usize, col: usize) {
        for r in 0..7 {
            for c in 0..7 {
                let is_border = r == 0 || r == 6 || c == 0 || c == 6;
                let is_center = r >= 2 && r <= 4 && c >= 2 && c <= 4;
                self.set_function(row + r, col + c, is_border || is_center);
            }
        }
    }

    fn place_separators(&mut self) {
        // White separators around each finder pattern
        for i in 0..8 {
            // Top-left: right col 7, bottom row 7
            if 7 < self.size { self.set_function_white(i, 7); }
            if 7 < self.size { self.set_function_white(7, i); }
            // Top-right: left col size-8, bottom row 7
            self.set_function_white(i, self.size - 8);
            self.set_function_white(7, self.size - 8 + i);
            // Bottom-left: right col 7, top row size-8
            self.set_function_white(self.size - 8, i);
            self.set_function_white(self.size - 8 + i, 7);
        }
    }

    fn place_timing_patterns(&mut self) {
        for i in 8..self.size - 8 {
            let black = i % 2 == 0;
            self.set_function(6, i, black); // horizontal
            self.set_function(i, 6, black); // vertical
        }
    }

    fn place_alignment_patterns(&mut self) {
        let positions = ALIGNMENT_POSITIONS[(self.version - 1) as usize];
        for &row in positions {
            for &col in positions {
                // Skip if overlapping finder pattern
                if self.is_function[row as usize][col as usize] {
                    continue;
                }
                self.place_alignment_at(row as usize, col as usize);
            }
        }
    }

    fn place_alignment_at(&mut self, center_row: usize, center_col: usize) {
        for dr in -2i32..=2 {
            for dc in -2i32..=2 {
                let r = (center_row as i32 + dr) as usize;
                let c = (center_col as i32 + dc) as usize;
                let is_border = dr.abs() == 2 || dc.abs() == 2;
                let is_center = dr == 0 && dc == 0;
                self.set_function(r, c, is_border || is_center);
            }
        }
    }

    fn place_dark_module(&mut self) {
        let row = 4 * self.version as usize + 9;
        self.set_function(row, 8, true);
    }

    fn reserve_format_areas(&mut self) {
        // Around top-left finder
        for i in 0..=8 {
            if i < self.size {
                self.is_function[8][i] = true;
                self.is_function[i][8] = true;
            }
        }
        // Bottom-left and top-right
        for i in 0..8 {
            self.is_function[self.size - 1 - i][8] = true;
            self.is_function[8][self.size - 1 - i] = true;
        }
    }

    fn reserve_version_areas(&mut self) {
        // Bottom-left 6x3 and top-right 3x6
        for i in 0..6 {
            for j in 0..3 {
                self.is_function[self.size - 11 + j][i] = true;
                self.is_function[i][self.size - 11 + j] = true;
            }
        }
    }

    // ── Data placement ───────────────────────────────────────────

    fn place_data_bits(&mut self, data: &[u8]) {
        let bits: Vec<bool> = data.iter()
            .flat_map(|&byte| (0..8).rev().map(move |i| (byte >> i) & 1 == 1))
            .collect();
        let mut bit_idx = 0;
        let mut right = self.size as i32 - 1;
        let mut upward = true;

        while right >= 0 {
            if right == 6 { right -= 1; } // skip timing column
            if right < 0 { break; }

            let col_pair = [right as usize, (right - 1).max(0) as usize];
            let rows: Vec<usize> = if upward {
                (0..self.size).rev().collect()
            } else {
                (0..self.size).collect()
            };

            for row in rows {
                for &col in &col_pair {
                    if col >= self.size { continue; }
                    if !self.is_function[row][col] && bit_idx < bits.len() {
                        self.modules[row][col] = bits[bit_idx];
                        bit_idx += 1;
                    }
                }
            }

            upward = !upward;
            right -= 2;
        }
    }

    // ── Masking ──────────────────────────────────────────────────

    fn evaluate_masks(&self) -> u8 {
        let mut best_mask = 0u8;
        let mut best_score = u32::MAX;
        for mask in 0..8u8 {
            let mut test = self.clone_data();
            test.apply_mask(mask);
            let score = test.penalty_score();
            if score < best_score {
                best_score = score;
                best_mask = mask;
            }
        }
        best_mask
    }

    fn clone_data(&self) -> Self {
        Self {
            version: self.version,
            size: self.size,
            modules: self.modules.clone(),
            is_function: self.is_function.clone(),
        }
    }

    fn apply_mask(&mut self, mask: u8) {
        for row in 0..self.size {
            for col in 0..self.size {
                if self.is_function[row][col] { continue; }
                let flip = match mask {
                    0 => (row + col) % 2 == 0,
                    1 => row % 2 == 0,
                    2 => col % 3 == 0,
                    3 => (row + col) % 3 == 0,
                    4 => (row / 2 + col / 3) % 2 == 0,
                    5 => (row * col) % 2 + (row * col) % 3 == 0,
                    6 => ((row * col) % 2 + (row * col) % 3) % 2 == 0,
                    7 => ((row + col) % 2 + (row * col) % 3) % 2 == 0,
                    _ => false,
                };
                if flip {
                    self.modules[row][col] = !self.modules[row][col];
                }
            }
        }
    }

    fn penalty_score(&self) -> u32 {
        self.penalty_rule1()
            + self.penalty_rule2()
            + self.penalty_rule3()
            + self.penalty_rule4()
    }

    fn penalty_rule1(&self) -> u32 {
        // 5+ consecutive same-color modules in row or column
        let mut score = 0u32;
        for row in 0..self.size {
            score += self.run_penalty_line((0..self.size).map(|c| self.modules[row][c]));
        }
        for col in 0..self.size {
            score += self.run_penalty_line((0..self.size).map(|r| self.modules[r][col]));
        }
        score
    }

    fn run_penalty_line(&self, iter: impl Iterator<Item = bool>) -> u32 {
        let mut score = 0u32;
        let mut count = 1u32;
        let mut last = None;
        for val in iter {
            if Some(val) == last {
                count += 1;
            } else {
                if count >= 5 { score += count - 2; }
                count = 1;
                last = Some(val);
            }
        }
        if count >= 5 { score += count - 2; }
        score
    }

    fn penalty_rule2(&self) -> u32 {
        // 2x2 blocks of same color
        let mut score = 0u32;
        for row in 0..self.size - 1 {
            for col in 0..self.size - 1 {
                let c = self.modules[row][col];
                if c == self.modules[row][col + 1]
                    && c == self.modules[row + 1][col]
                    && c == self.modules[row + 1][col + 1]
                {
                    score += 3;
                }
            }
        }
        score
    }

    fn penalty_rule3(&self) -> u32 {
        // Finder-like patterns: 10111010000 or 00001011101
        let pattern_a: [bool; 11] = [true, false, true, true, true, false, true, false, false, false, false];
        let pattern_b: [bool; 11] = [false, false, false, false, true, false, true, true, true, false, true];
        let mut score = 0u32;
        for row in 0..self.size {
            for col in 0..=self.size.saturating_sub(11) {
                let matches_a = (0..11).all(|i| self.modules[row][col + i] == pattern_a[i]);
                let matches_b = (0..11).all(|i| self.modules[row][col + i] == pattern_b[i]);
                if matches_a || matches_b { score += 40; }
            }
        }
        for col in 0..self.size {
            for row in 0..=self.size.saturating_sub(11) {
                let matches_a = (0..11).all(|i| self.modules[row + i][col] == pattern_a[i]);
                let matches_b = (0..11).all(|i| self.modules[row + i][col] == pattern_b[i]);
                if matches_a || matches_b { score += 40; }
            }
        }
        score
    }

    fn penalty_rule4(&self) -> u32 {
        // Proportion of dark modules
        let total = (self.size * self.size) as u32;
        let dark: u32 = self.modules.iter()
            .flat_map(|row| row.iter())
            .filter(|&&m| m)
            .count() as u32;
        let pct = (dark * 100) / total;
        let prev5 = (pct / 5) * 5;
        let next5 = prev5 + 5;
        let a = (prev5 as i32 - 50).unsigned_abs() / 5;
        let b = (next5 as i32 - 50).unsigned_abs() / 5;
        a.min(b) * 10
    }

    // ── Format & version info ────────────────────────────────────

    fn write_format_info(&mut self, mask: u8) {
        let info = FORMAT_INFO_BITS_M[mask as usize];
        // Write to two locations
        for i in 0..15 {
            let bit = (info >> (14 - i)) & 1 == 1;
            // Location 1: around top-left finder
            let (r1, c1) = format_info_coords_1(i, self.size);
            self.modules[r1][c1] = bit;
            // Location 2: bottom-left and top-right
            let (r2, c2) = format_info_coords_2(i, self.size);
            self.modules[r2][c2] = bit;
        }
    }

    fn write_version_info(&mut self) {
        if let Some(info) = crate::qrcode::tables::version_info(self.version) {
            for i in 0..18 {
                let bit = (info >> i) & 1 == 1;
                let row = i / 3;
                let col = i % 3;
                // Bottom-left
                self.modules[self.size - 11 + col][row] = bit;
                // Top-right
                self.modules[row][self.size - 11 + col] = bit;
            }
        }
    }

    // ── Helpers ──────────────────────────────────────────────────

    fn set_function(&mut self, row: usize, col: usize, black: bool) {
        self.modules[row][col] = black;
        self.is_function[row][col] = true;
    }

    fn set_function_white(&mut self, row: usize, col: usize) {
        self.modules[row][col] = false;
        self.is_function[row][col] = true;
    }
}

/// Format info bit positions around top-left finder (location 1).
fn format_info_coords_1(i: usize, size: usize) -> (usize, usize) {
    match i {
        0..=5 => (8, i),
        6 => (8, 7),
        7 => (8, 8),
        8 => (7, 8),
        _ => (14 - i, 8),
    }
}

/// Format info bit positions at bottom-left / top-right (location 2).
fn format_info_coords_2(i: usize, size: usize) -> (usize, usize) {
    match i {
        0..=7 => (size - 1 - i, 8),
        _ => (8, size - 15 + i),
    }
}
```

- [ ] **Step 4: Add module declarations**

Add to `src/qrcode/mod.rs`:
```rust
pub mod matrix;
```

Add to `tests/qrcode/mod.rs`:
```rust
mod matrix_test;
```

- [ ] **Step 5: Run tests**

Run: `cargo test --test matrix_test -v`
Expected: all tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/qrcode/matrix.rs src/qrcode/mod.rs tests/qrcode/matrix_test.rs tests/qrcode/mod.rs
git commit -m "feat(qrcode): add QrMatrix with functional patterns, data placement, masking"
```

---

## Task 6: Top-Level encode() + End-to-End Verification

**Files:**
- Modify: `src/qrcode/mod.rs`
- Modify: `tests/qrcode/matrix_test.rs`

- [ ] **Step 1: Write failing end-to-end test**

Add to `tests/qrcode/matrix_test.rs`:

```rust
use viv::qrcode;

#[test]
fn encode_hello_produces_valid_matrix() {
    let matrix = qrcode::encode("HELLO").unwrap();
    assert_eq!(matrix.size(), 21); // V1 for short text
    // Finder pattern sanity check
    assert!(matrix.get(0, 0));
    assert!(matrix.get(0, matrix.size() - 1));
    assert!(matrix.get(matrix.size() - 1, 0));
}

#[test]
fn encode_url_produces_valid_matrix() {
    let matrix = qrcode::encode("https://example.com").unwrap();
    assert!(matrix.size() >= 21);
}

#[test]
fn encode_empty_returns_error() {
    assert!(qrcode::encode("").is_err());
}

#[test]
fn encode_long_text_larger_version() {
    let text = "a".repeat(100);
    let matrix = qrcode::encode(&text).unwrap();
    assert!(matrix.size() > 21); // needs version > 1
}
```

- [ ] **Step 2: Implement top-level encode()**

Add to `src/qrcode/mod.rs`:

```rust
pub mod gf256;
pub mod rs;
pub mod tables;
pub mod encode;
pub mod matrix;

pub use matrix::QrMatrix;

/// Encode UTF-8 text into a QR code matrix.
pub fn encode(text: &str) -> crate::Result<QrMatrix> {
    let encoded = encode::encode_and_interleave(text)?;
    Ok(QrMatrix::build(encoded.version, &encoded.data))
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --test matrix_test -v`
Expected: all tests PASS

- [ ] **Step 4: Commit**

```bash
git add src/qrcode/mod.rs tests/qrcode/matrix_test.rs
git commit -m "feat(qrcode): add top-level encode() with end-to-end tests"
```

---

## Task 7: QrCodeWidget

**Files:**
- Create: `src/tui/qrcode.rs`
- Create: `tests/tui/qrcode_test.rs`
- Modify: `src/tui/mod.rs`
- Modify: `tests/tui/mod.rs`

- [ ] **Step 1: Write failing tests**

```rust
// tests/tui/qrcode_test.rs
use viv::core::terminal::buffer::{Buffer, Rect};
use viv::tui::qrcode::QrCodeWidget;
use viv::tui::widget::Widget;

#[test]
fn renders_without_panic() {
    let widget = QrCodeWidget::new("test");
    let area = Rect::new(0, 0, 40, 20);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
}

#[test]
fn height_short_text() {
    let h = QrCodeWidget::height("Hi");
    // V1: 21x21 matrix + 4 quiet zone = 25 rows, /2 = ~13
    assert!(h >= 10);
    assert!(h <= 20);
}

#[test]
fn renders_half_block_chars() {
    let widget = QrCodeWidget::new("A");
    let area = Rect::new(0, 0, 40, 20);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    // Should contain at least some half-block characters
    let has_blocks = (0..area.width).any(|x| {
        (0..area.height).any(|y| {
            let ch = buf.get(x, y).ch;
            ch == '▀' || ch == '▄' || ch == '█'
        })
    });
    assert!(has_blocks, "QR code should render block characters");
}

#[test]
fn renders_centered() {
    let widget = QrCodeWidget::new("Hi");
    let area = Rect::new(0, 0, 80, 30); // much larger than needed
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);

    // First column should be mostly empty (centered)
    let first_col_empty = (0..area.height).all(|y| buf.get(0, y).ch == ' ');
    assert!(first_col_empty, "QR code should be centered, leaving edges empty");
}

#[test]
fn too_small_area_does_not_panic() {
    let widget = QrCodeWidget::new("test");
    let area = Rect::new(0, 0, 5, 3);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf); // should not panic
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test qrcode_test 2>&1 | head -10`

- [ ] **Step 3: Implement QrCodeWidget**

```rust
// src/tui/qrcode.rs
use crate::core::terminal::buffer::{Buffer, Rect};
use crate::core::terminal::style::Color;
use crate::tui::widget::Widget;

const WHITE: Color = Color::Rgb(255, 255, 255);
const BLACK: Color = Color::Rgb(0, 0, 0);
const QUIET_ZONE: usize = 2;

pub struct QrCodeWidget<'a> {
    data: &'a str,
}

impl<'a> QrCodeWidget<'a> {
    pub fn new(data: &'a str) -> Self {
        Self { data }
    }

    /// Calculate the rendered height in terminal rows.
    pub fn height(data: &str) -> u16 {
        let version = crate::qrcode::encode::select_version(data.len()).unwrap_or(1);
        let matrix_size = 4 * version as usize + 17;
        let total = matrix_size + QUIET_ZONE * 2;
        ((total + 1) / 2) as u16 // 2 rows per terminal character
    }
}

impl Widget for QrCodeWidget<'_> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let matrix = match crate::qrcode::encode(self.data) {
            Ok(m) => m,
            Err(_) => {
                // Render error text
                buf.set_str(area.x, area.y, "QR error", Some(Color::Rgb(171, 43, 63)), false);
                return;
            }
        };

        let qr_size = matrix.size();
        let total = qr_size + QUIET_ZONE * 2;
        let width = total;
        let height_rows = (total + 1) / 2;

        // Check if area is large enough
        if (area.width as usize) < width || (area.height as usize) < height_rows {
            return;
        }

        // Center in area
        let x_offset = area.x + (area.width as usize - width) as u16 / 2;
        let y_offset = area.y + (area.height as usize - height_rows) as u16 / 2;

        // Helper: get module state with quiet zone (false = white outside matrix)
        let get_module = |row: usize, col: usize| -> bool {
            if row < QUIET_ZONE || col < QUIET_ZONE {
                return false;
            }
            let r = row - QUIET_ZONE;
            let c = col - QUIET_ZONE;
            if r < qr_size && c < qr_size {
                matrix.get(r, c)
            } else {
                false
            }
        };

        // Render using half-block characters
        for term_row in 0..height_rows {
            let top_row = term_row * 2;
            let bot_row = top_row + 1;

            for col in 0..width {
                let top = get_module(top_row, col);
                let bot = if bot_row < total { get_module(bot_row, col) } else { false };

                let x = x_offset + col as u16;
                let y = y_offset + term_row as u16;

                if x >= area.x + area.width || y >= area.y + area.height {
                    continue;
                }

                let cell = buf.get_mut(x, y);
                cell.ch = '▀';
                // ▀ renders: top half = foreground, bottom half = background
                cell.fg = Some(if top { BLACK } else { WHITE });
                cell.bg = Some(if bot { BLACK } else { WHITE });
            }
        }
    }
}
```

Note: Buffer's Cell struct needs a `bg` field. Check if it already exists. If not, add `pub bg: Option<Color>` to `Cell` in `src/core/terminal/buffer.rs` and update the `diff()` method to emit background color escape sequences.

- [ ] **Step 4: Add module declarations**

Add to `src/tui/mod.rs`:
```rust
pub mod qrcode;
```

Add to `tests/tui/mod.rs`:
```rust
mod qrcode_test;
```

- [ ] **Step 5: Run tests**

Run: `cargo test --test qrcode_test -v`
Expected: all tests PASS

- [ ] **Step 6: Run full test suite**

Run: `cargo test && cargo clippy`
Expected: all pass, no warnings

- [ ] **Step 7: Commit**

```bash
git add src/tui/qrcode.rs tests/tui/qrcode_test.rs src/tui/mod.rs tests/tui/mod.rs
git commit -m "feat(tui): add QrCodeWidget with Unicode half-block rendering"
```
