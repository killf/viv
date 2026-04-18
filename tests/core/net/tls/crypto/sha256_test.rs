// SHA-256 / getrandom tests

#[test]
fn getrandom_fills_buffer() {
    let mut buf = [0u8; 32];
    viv::core::net::tls::crypto::getrandom(&mut buf).unwrap();
    assert_ne!(buf, [0u8; 32]);
}
