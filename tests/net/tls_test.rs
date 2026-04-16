use viv::net::tcp;

#[test]
fn tcp_connect_to_nowhere_fails() {
    let result = tcp::connect("127.0.0.1", 19999);
    assert!(result.is_err());
}

#[test]
fn tls_openssl_links() {
    use viv::net::tls::TlsStream;
    let _size = std::mem::size_of::<TlsStream>();
}

/// Test real TCP connection to 8.8.8.8:53 (Google DNS)
#[cfg(feature = "full_test")]
#[test]
fn tcp_connect_real_server() {
    let stream = tcp::connect("8.8.8.8", 53);
    assert!(stream.is_ok(), "TCP connect to 8.8.8.8:53 failed: {:?}", stream.err());
}

/// Test real HTTPS GET to www.baidu.com
#[cfg(feature = "full_test")]
#[test]
fn http_get_baidu() {
    use viv::net::tls::TlsStream;
    use viv::net::http::HttpRequest;
    use std::io::{Read, Write};

    let mut tls = TlsStream::connect("www.baidu.com", 443)
        .expect("TLS connect to www.baidu.com failed");

    let req = HttpRequest {
        method: "GET".into(),
        path: "/".into(),
        headers: vec![
            ("Host".into(), "www.baidu.com".into()),
            ("Connection".into(), "close".into()),
        ],
        body: None,
    };

    tls.write_all(&req.to_bytes()).expect("write failed");

    let mut response = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = tls.read(&mut buf).unwrap_or(0);
        if n == 0 { break; }
        response.extend_from_slice(&buf[..n]);
    }

    let resp_str = String::from_utf8_lossy(&response);
    assert!(resp_str.starts_with("HTTP/1.1"), "Expected HTTP response, got: {}", &resp_str[..50.min(resp_str.len())]);
    println!("baidu response: {} bytes, status line: {}", response.len(), resp_str.lines().next().unwrap_or(""));
}
