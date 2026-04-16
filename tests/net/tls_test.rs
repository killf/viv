use viv::net::tcp;

#[test]
fn tcp_connect_to_nowhere_fails() {
    let result = tcp::connect("127.0.0.1", 19999);
    assert!(result.is_err());
}

#[test]
fn tls_openssl_links() {
    // Just verify OpenSSL can be loaded without crashing
    // (actual TLS test would need a real server)
    use viv::net::tls::TlsStream;
    // TlsStream type exists and is usable
    let _size = std::mem::size_of::<TlsStream>();
}
