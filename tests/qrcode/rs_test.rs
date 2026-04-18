use viv::qrcode::rs;

#[test]
fn generator_poly_length() {
    let gpoly = rs::generator_poly(7);
    assert_eq!(gpoly.len(), 8); // degree 7 → 8 coefficients
    assert_eq!(gpoly[0], 1); // monic polynomial
}

#[test]
fn generator_poly_10() {
    let gpoly = rs::generator_poly(10);
    assert_eq!(gpoly.len(), 11);
    assert_eq!(gpoly[0], 1);
}

#[test]
fn rs_encode_hello_world_1m() {
    // CRITICAL known test vector from thonky.com:
    // "HELLO WORLD" as 1-M QR code
    // Data codewords: [32, 91, 11, 120, 209, 114, 220, 77, 67, 64, 236, 17, 236, 17, 236, 17]
    // Expected ECC (10 codewords): [196, 35, 39, 119, 235, 215, 231, 226, 93, 23]
    let data = vec![
        32, 91, 11, 120, 209, 114, 220, 77, 67, 64, 236, 17, 236, 17, 236, 17,
    ];
    let ecc = rs::rs_encode(&data, 10);
    assert_eq!(ecc, vec![196, 35, 39, 119, 235, 215, 231, 226, 93, 23]);
}

#[test]
fn rs_encode_returns_correct_length() {
    let data = vec![1, 2, 3, 4, 5];
    let ecc = rs::rs_encode(&data, 7);
    assert_eq!(ecc.len(), 7);
}

#[test]
fn rs_encode_all_zeros() {
    let data = vec![0; 10];
    let ecc = rs::rs_encode(&data, 5);
    assert_eq!(ecc, vec![0; 5]);
}

#[test]
fn rs_encode_single_byte() {
    let data = vec![42];
    let ecc = rs::rs_encode(&data, 4);
    assert_eq!(ecc.len(), 4);
    // Non-trivial: at least some ECC bytes should be non-zero
    assert!(ecc.iter().any(|&b| b != 0));
}
