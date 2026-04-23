use crate::tui::qrcode::gf256::{EXP_TABLE, mul};

/// Generate the Reed-Solomon generator polynomial for `ecc_count` error correction codewords.
///
/// g(x) = (x + α^0)(x + α^1)...(x + α^(ecc_count-1))
///
/// Returns a monic polynomial of degree `ecc_count` with `ecc_count + 1` coefficients.
/// Coefficients are stored from highest degree to lowest: `[leading=1, ..., constant]`.
pub fn generator_poly(ecc_count: usize) -> Vec<u8> {
    // Start with poly = [1] representing the constant polynomial 1
    let mut poly = vec![1u8];

    for alpha_i in EXP_TABLE.iter().take(ecc_count) {
        // Multiply poly by (x + α^i)
        // new_poly has one more term than poly
        let mut new_poly = vec![0u8; poly.len() + 1];

        for j in 0..poly.len() {
            // poly[j] * x  contributes to new_poly[j]
            new_poly[j] ^= poly[j];
            // poly[j] * α^i contributes to new_poly[j+1]
            new_poly[j + 1] ^= mul(poly[j], *alpha_i);
        }

        poly = new_poly;
    }

    poly
}

/// Compute error correction codewords via polynomial long division.
///
/// Returns `ecc_count` ECC bytes for the given `data` slice.
pub fn rs_encode(data: &[u8], ecc_count: usize) -> Vec<u8> {
    let gpoly = generator_poly(ecc_count);
    let mut remainder = vec![0u8; ecc_count];

    for &byte in data {
        let factor = byte ^ remainder[0];

        // Shift remainder left by 1
        for i in 0..ecc_count - 1 {
            remainder[i] = remainder[i + 1];
        }
        remainder[ecc_count - 1] = 0;

        // XOR with generator polynomial scaled by factor
        // gpoly[0] is the leading coefficient (1), gpoly[1..] are the remaining coefficients
        for j in 0..ecc_count {
            remainder[j] ^= mul(gpoly[j + 1], factor);
        }
    }

    remainder
}
