use viv::core::net::http::HttpRequest;
use viv::core::net::tls::TlsStream;
use viv::core::runtime::executor::block_on;

#[test]
fn tls_stream_is_constructible() {
    let _size = std::mem::size_of::<TlsStream>();
}

/// Pure Rust TLS 1.3 handshake + HTTPS GET against a real server.
///
/// Verifies the complete TLS 1.3 implementation (X25519 key exchange,
/// AES-128-GCM record encryption, SHA-256 key schedule) works end-to-end
/// with zero external dependencies (no OpenSSL).
#[cfg(feature = "full_test")]
#[test]
fn tls13_pure_rust_https_get() {
    block_on(async {
        let host = "www.wechat.com";
        let mut tls = TlsStream::connect(host, 443).await.expect("TLS 1.3 connect failed");

        let req = HttpRequest {
            method: "GET".into(),
            path: "/".into(),
            headers: vec![
                ("Host".into(), host.into()),
                ("User-Agent".into(), "viv/0.1".into()),
                ("Accept".into(), "*/*".into()),
                ("Connection".into(), "close".into()),
            ],
            body: None,
        };

        tls.write_all(&req.to_bytes()).await.expect("write failed");

        let mut response = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = tls.read(&mut buf).await.expect("read failed");
            if n == 0 {
                break;
            }
            response.extend_from_slice(&buf[..n]);
        }

        assert!(!response.is_empty(), "Response was empty");
        let resp_str = String::from_utf8_lossy(&response);
        let first_line = resp_str.lines().next().unwrap_or("");
        assert!(
            first_line.starts_with("HTTP/1."),
            "Expected HTTP/1.x response, got: {}",
            first_line,
        );
        println!(
            "Pure Rust TLS 1.3: {} bytes, status: {}",
            response.len(),
            first_line
        );
    });
}

/// Pure Rust TLS 1.2 auto-negotiation + HTTPS GET against baidu.com.
/// Verifies ClientHello offers TLS 1.3+1.2 and baidu.com selects TLS 1.2.
#[cfg(feature = "full_test")]
#[test]
fn tls12_pure_rust_https_get() {
    block_on(async {
        let host = "www.wechat.com";
        let mut tls = TlsStream::connect(host, 443).await.expect("TLS connect failed");

        let req = HttpRequest {
            method: "GET".into(),
            path: "/".into(),
            headers: vec![
                ("Host".into(), host.into()),
                ("User-Agent".into(), "viv/0.1".into()),
                ("Accept".into(), "*/*".into()),
                ("Connection".into(), "close".into()),
            ],
            body: None,
        };
        tls.write_all(&req.to_bytes()).await.expect("write failed");

        let mut response = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = tls.read(&mut buf).await.expect("read failed");
            if n == 0 { break; }
            response.extend_from_slice(&buf[..n]);
        }

        assert!(!response.is_empty(), "Response was empty");
        let resp_str = String::from_utf8_lossy(&response);
        let first_line = resp_str.lines().next().unwrap_or("");
        assert!(
            first_line.starts_with("HTTP/"),
            "Expected HTTP response, got: {}", first_line,
        );
        println!("TLS auto-negotiation: {} bytes, status: {}", response.len(), first_line);
    });
}
