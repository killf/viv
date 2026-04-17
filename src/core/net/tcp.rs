use std::net::TcpStream;

pub fn connect(host: &str, port: u16) -> crate::Result<TcpStream> {
    let stream = TcpStream::connect((host, port))?;
    Ok(stream)
}
