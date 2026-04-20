use viv::tui::qrcode::gf256;

#[test]
fn exp_table_first_entries() {
    assert_eq!(gf256::EXP_TABLE[0], 1);
    assert_eq!(gf256::EXP_TABLE[1], 2);
    assert_eq!(gf256::EXP_TABLE[2], 4);
    assert_eq!(gf256::EXP_TABLE[7], 128);
}

#[test]
fn exp_table_wraps_at_8() {
    // α^8 = 29 (0x1D) because x^8 mod (x^8+x^4+x^3+x^2+1) = x^4+x^3+x^2+1 = 16+8+4+1 = 29
    assert_eq!(gf256::EXP_TABLE[8], 29);
}

#[test]
fn log_exp_inverse() {
    for i in 0u16..255 {
        let exp_val = gf256::EXP_TABLE[i as usize];
        assert_eq!(gf256::LOG_TABLE[exp_val as usize] as u16, i);
    }
}

#[test]
fn mul_zero() {
    assert_eq!(gf256::mul(0, 5), 0);
    assert_eq!(gf256::mul(5, 0), 0);
    assert_eq!(gf256::mul(0, 0), 0);
}

#[test]
fn mul_identity() {
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
fn mul_commutative() {
    for a in [1u8, 2, 17, 128, 255] {
        for b in [1u8, 3, 42, 200, 254] {
            assert_eq!(gf256::mul(a, b), gf256::mul(b, a));
        }
    }
}

#[test]
fn div_zero_numerator() {
    assert_eq!(gf256::div(0, 5), 0);
}

#[test]
fn div_by_one() {
    assert_eq!(gf256::div(7, 1), 7);
    assert_eq!(gf256::div(255, 1), 255);
}

#[test]
fn mul_div_roundtrip() {
    for a in 1..=255u8 {
        for &b in &[1u8, 2, 37, 128, 255] {
            assert_eq!(gf256::div(gf256::mul(a, b), b), a);
        }
    }
}

#[test]
fn pow_zero_exponent() {
    assert_eq!(gf256::pow(2, 0), 1);
    assert_eq!(gf256::pow(255, 0), 1);
}

#[test]
fn pow_one_exponent() {
    assert_eq!(gf256::pow(2, 1), 2);
    assert_eq!(gf256::pow(42, 1), 42);
}

#[test]
fn pow_matches_exp_table() {
    assert_eq!(gf256::pow(2, 8), gf256::EXP_TABLE[8]); // 29
}
