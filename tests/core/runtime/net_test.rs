use std::net::TcpListener;
use std::thread;
use viv::core::runtime::executor::block_on;
use viv::core::net::async_tcp::AsyncTcpStream;

fn start_echo_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    thread::spawn(move || {
        if let Ok((mut conn, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            use std::io::{Read, Write};
            if let Ok(n) = conn.read(&mut buf) {
                conn.write_all(&buf[..n]).ok();
            }
        }
    });
    port
}

#[test]
fn async_tcp_write_and_read() {
    let port = start_echo_server();

    block_on(async move {
        let mut stream = AsyncTcpStream::connect("127.0.0.1", port).await.unwrap();
        stream.write_all(b"hello").await.unwrap();
        let mut buf = [0u8; 5];
        let n = stream.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], b"hello");
    });
}
