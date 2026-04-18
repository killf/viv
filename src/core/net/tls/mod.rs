// TLS 1.3 pure-Rust implementation
//
// Provides TlsStream (sync) and AsyncTlsStream (async) that implement
// the full TLS 1.3 handshake and record layer using only Rust code
// with zero external dependencies.

pub mod codec;
pub mod crypto;
pub mod handshake;
pub mod key_schedule;
pub mod record;
pub mod x509;

use std::io::{self, Read, Write};

use super::tcp;
use codec::{ALERT, CHANGE_CIPHER_SPEC, HANDSHAKE};
use handshake::{Handshake, HandshakeResult};
use record::RecordLayer;

// ── TlsStream (sync) ──────────────────────────────────────────────

pub struct TlsStream {
    tcp: std::net::TcpStream,
    record: RecordLayer,
    /// Raw bytes read from TCP but not yet parsed as records.
    read_buf: Vec<u8>,
    /// Decrypted application data ready for the caller.
    plaintext_buf: Vec<u8>,
}

unsafe impl Send for TlsStream {}

impl TlsStream {
    /// Connect to `host:port` over TLS 1.3.
    pub fn connect(host: &str, port: u16) -> crate::Result<Self> {
        let mut tcp_stream = tcp::connect(host, port)?;
        let mut record = RecordLayer::new();
        let mut hs = Handshake::new(host)?;

        // 1. Encode and send ClientHello
        let ch_msg = hs.encode_client_hello()?;
        let mut ch_record = Vec::new();
        record.write_plaintext(HANDSHAKE, &ch_msg, &mut ch_record);
        tcp_stream.write_all(&ch_record).map_err(crate::Error::Io)?;

        // 2. Read server messages until handshake completes
        let mut read_buf = Vec::new();
        let mut tmp = [0u8; 8192];

        'outer: loop {
            // Try to parse records from what we have
            loop {
                if read_buf.len() < 5 {
                    break; // need more data to read record header
                }
                let rec_len = ((read_buf[3] as usize) << 8) | (read_buf[4] as usize);
                if read_buf.len() < 5 + rec_len {
                    break; // need more data for record body
                }

                let (ct, payload, consumed) = record.read_record(&read_buf)?;

                // Remove consumed bytes
                read_buf.drain(..consumed);

                match ct {
                    CHANGE_CIPHER_SPEC => {
                        // Middlebox compatibility -- just skip
                        continue;
                    }
                    ALERT => {
                        if payload.len() >= 2 {
                            return Err(crate::Error::Tls(format!(
                                "TLS alert: level={} desc={}",
                                payload[0], payload[1]
                            )));
                        }
                        return Err(crate::Error::Tls("TLS alert received".into()));
                    }
                    HANDSHAKE => {
                        // May contain multiple handshake messages
                        let mut pos = 0;
                        while pos < payload.len() {
                            if pos + 4 > payload.len() {
                                return Err(crate::Error::Tls(
                                    "truncated handshake in record".into(),
                                ));
                            }
                            let msg_len = ((payload[pos + 1] as usize) << 16)
                                | ((payload[pos + 2] as usize) << 8)
                                | (payload[pos + 3] as usize);
                            let msg_end = pos + 4 + msg_len;
                            if msg_end > payload.len() {
                                return Err(crate::Error::Tls(
                                    "handshake message exceeds record".into(),
                                ));
                            }

                            let msg_bytes = &payload[pos..msg_end];
                            let result = hs.handle_message(msg_bytes, &mut record)?;

                            pos = msg_end;

                            if let HandshakeResult::Complete = result {
                                break 'outer;
                            }
                        }
                    }
                    _ => {
                        return Err(crate::Error::Tls(format!(
                            "unexpected record type during handshake: 0x{:02x}",
                            ct
                        )));
                    }
                }
            }

            // Need more data from TCP
            let n = tcp_stream.read(&mut tmp).map_err(crate::Error::Io)?;
            if n == 0 {
                return Err(crate::Error::Tls(
                    "connection closed during handshake".into(),
                ));
            }
            read_buf.extend_from_slice(&tmp[..n]);
        }

        // 3. Send ChangeCipherSpec (compatibility)
        let mut ccs = Vec::new();
        codec::encode_change_cipher_spec(&mut ccs);
        tcp_stream.write_all(&ccs).map_err(crate::Error::Io)?;

        // 4. Send client Finished (encrypted with handshake keys)
        let finished_msg = hs.encode_client_finished();
        let mut finished_record = Vec::new();
        record.write_encrypted(HANDSHAKE, &finished_msg, &mut finished_record);
        tcp_stream
            .write_all(&finished_record)
            .map_err(crate::Error::Io)?;

        // 5. Install application keys
        hs.install_app_keys(&mut record);

        Ok(TlsStream {
            tcp: tcp_stream,
            record,
            read_buf,
            plaintext_buf: Vec::new(),
        })
    }
}

impl Read for TlsStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // If we have buffered plaintext, return from that first
        if !self.plaintext_buf.is_empty() {
            let n = core::cmp::min(buf.len(), self.plaintext_buf.len());
            buf[..n].copy_from_slice(&self.plaintext_buf[..n]);
            self.plaintext_buf.drain(..n);
            return Ok(n);
        }

        // Read records until we get application data
        let mut tmp = [0u8; 8192];
        loop {
            // Try to parse a record from read_buf
            if self.read_buf.len() >= 5 {
                let rec_len = ((self.read_buf[3] as usize) << 8) | (self.read_buf[4] as usize);
                if self.read_buf.len() >= 5 + rec_len {
                    let (ct, payload, consumed) = self
                        .record
                        .read_record(&self.read_buf)
                        .map_err(|e| io::Error::other(e.to_string()))?;
                    self.read_buf.drain(..consumed);

                    match ct {
                        codec::APPLICATION_DATA => {
                            if payload.len() <= buf.len() {
                                buf[..payload.len()].copy_from_slice(&payload);
                                return Ok(payload.len());
                            } else {
                                buf.copy_from_slice(&payload[..buf.len()]);
                                self.plaintext_buf.extend_from_slice(&payload[buf.len()..]);
                                return Ok(buf.len());
                            }
                        }
                        ALERT => {
                            if payload.len() >= 2 && payload[0] == 1 && payload[1] == 0 {
                                // close_notify -- treat as EOF
                                return Ok(0);
                            }
                            return Err(io::Error::other(format!(
                                "TLS alert: level={} desc={}",
                                payload.first().unwrap_or(&0),
                                payload.get(1).unwrap_or(&0),
                            )));
                        }
                        HANDSHAKE => {
                            // Post-handshake messages (NewSessionTicket, etc.) -- skip
                            continue;
                        }
                        _ => {
                            continue;
                        }
                    }
                }
            }

            // Need more data
            let n = self.tcp.read(&mut tmp)?;
            if n == 0 {
                return Ok(0); // TCP EOF
            }
            self.read_buf.extend_from_slice(&tmp[..n]);
        }
    }
}

impl Write for TlsStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Fragment into <=16384 byte records
        let chunk_size = 16384;
        let to_send = core::cmp::min(buf.len(), chunk_size);
        let mut encrypted = Vec::new();
        self.record
            .write_encrypted(codec::APPLICATION_DATA, &buf[..to_send], &mut encrypted);
        self.tcp.write_all(&encrypted)?;
        Ok(to_send)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.tcp.flush()
    }
}

impl Drop for TlsStream {
    fn drop(&mut self) {
        // Send close_notify alert
        let mut alert_record = Vec::new();
        self.record.write_encrypted(
            ALERT,
            &[1, 0], // warning(1), close_notify(0)
            &mut alert_record,
        );
        let _ = self.tcp.write_all(&alert_record);
    }
}

// ── AsyncTlsStream ─────────────────────────────────────────────────

use super::async_tcp::AsyncTcpStream;

pub struct AsyncTlsStream {
    tcp: AsyncTcpStream,
    record: RecordLayer,
    read_buf: Vec<u8>,
    plaintext_buf: Vec<u8>,
}

unsafe impl Send for AsyncTlsStream {}

impl AsyncTlsStream {
    /// Connect to `host:port` over TLS 1.3 with async I/O.
    pub async fn connect(host: &str, port: u16) -> crate::Result<Self> {
        let mut tcp = AsyncTcpStream::connect(host, port).await?;
        let fd = tcp.raw_handle();
        let mut record = RecordLayer::new();
        let mut hs = Handshake::new(host)?;

        // 1. Send ClientHello
        let ch_msg = hs.encode_client_hello()?;
        let mut ch_record = Vec::new();
        record.write_plaintext(HANDSHAKE, &ch_msg, &mut ch_record);
        async_write_all(&mut tcp, fd, &ch_record).await?;

        // 2. Read server messages
        let mut read_buf = Vec::new();
        let mut tmp = [0u8; 8192];

        'outer: loop {
            loop {
                if read_buf.len() < 5 {
                    break;
                }
                let rec_len = ((read_buf[3] as usize) << 8) | (read_buf[4] as usize);
                if read_buf.len() < 5 + rec_len {
                    break;
                }

                let (ct, payload, consumed) = record.read_record(&read_buf)?;
                read_buf.drain(..consumed);

                match ct {
                    CHANGE_CIPHER_SPEC => continue,
                    ALERT => {
                        if payload.len() >= 2 {
                            return Err(crate::Error::Tls(format!(
                                "TLS alert: level={} desc={}",
                                payload[0], payload[1]
                            )));
                        }
                        return Err(crate::Error::Tls("TLS alert received".into()));
                    }
                    HANDSHAKE => {
                        let mut pos = 0;
                        while pos < payload.len() {
                            if pos + 4 > payload.len() {
                                return Err(crate::Error::Tls(
                                    "truncated handshake in record".into(),
                                ));
                            }
                            let msg_len = ((payload[pos + 1] as usize) << 16)
                                | ((payload[pos + 2] as usize) << 8)
                                | (payload[pos + 3] as usize);
                            let msg_end = pos + 4 + msg_len;
                            if msg_end > payload.len() {
                                return Err(crate::Error::Tls(
                                    "handshake message exceeds record".into(),
                                ));
                            }

                            let msg_bytes = &payload[pos..msg_end];
                            let result = hs.handle_message(msg_bytes, &mut record)?;
                            pos = msg_end;

                            if let HandshakeResult::Complete = result {
                                break 'outer;
                            }
                        }
                    }
                    _ => {
                        return Err(crate::Error::Tls(format!(
                            "unexpected record type during handshake: 0x{:02x}",
                            ct
                        )));
                    }
                }
            }

            // Need more data
            let n = async_read(&mut tcp, fd, &mut tmp).await?;
            if n == 0 {
                return Err(crate::Error::Tls(
                    "connection closed during handshake".into(),
                ));
            }
            read_buf.extend_from_slice(&tmp[..n]);
        }

        // 3. Send CCS
        let mut ccs = Vec::new();
        codec::encode_change_cipher_spec(&mut ccs);
        async_write_all(&mut tcp, fd, &ccs).await?;

        // 4. Send client Finished
        let finished_msg = hs.encode_client_finished();
        let mut finished_record = Vec::new();
        record.write_encrypted(HANDSHAKE, &finished_msg, &mut finished_record);
        async_write_all(&mut tcp, fd, &finished_record).await?;

        // 5. Install app keys
        hs.install_app_keys(&mut record);

        Ok(AsyncTlsStream {
            tcp,
            record,
            read_buf,
            plaintext_buf: Vec::new(),
        })
    }

    pub fn raw_handle(&self) -> crate::core::platform::RawHandle {
        self.tcp.raw_handle()
    }

    /// Async read into buffer. Returns number of bytes read (0 = EOF).
    pub async fn read(&mut self, buf: &mut [u8]) -> crate::Result<usize> {
        // Return buffered plaintext first
        if !self.plaintext_buf.is_empty() {
            let n = core::cmp::min(buf.len(), self.plaintext_buf.len());
            buf[..n].copy_from_slice(&self.plaintext_buf[..n]);
            self.plaintext_buf.drain(..n);
            return Ok(n);
        }

        let fd = self.tcp.raw_handle();
        let mut tmp = [0u8; 8192];

        loop {
            // Try to parse a record
            if self.read_buf.len() >= 5 {
                let rec_len = ((self.read_buf[3] as usize) << 8) | (self.read_buf[4] as usize);
                if self.read_buf.len() >= 5 + rec_len {
                    let (ct, payload, consumed) = self.record.read_record(&self.read_buf)?;
                    self.read_buf.drain(..consumed);

                    match ct {
                        codec::APPLICATION_DATA => {
                            if payload.len() <= buf.len() {
                                buf[..payload.len()].copy_from_slice(&payload);
                                return Ok(payload.len());
                            } else {
                                buf.copy_from_slice(&payload[..buf.len()]);
                                self.plaintext_buf.extend_from_slice(&payload[buf.len()..]);
                                return Ok(buf.len());
                            }
                        }
                        ALERT => {
                            if payload.len() >= 2 && payload[0] == 1 && payload[1] == 0 {
                                return Ok(0);
                            }
                            return Err(crate::Error::Tls(format!(
                                "TLS alert: level={} desc={}",
                                payload.first().unwrap_or(&0),
                                payload.get(1).unwrap_or(&0),
                            )));
                        }
                        _ => continue,
                    }
                }
            }

            let n = async_read(&mut self.tcp, fd, &mut tmp).await?;
            if n == 0 {
                return Ok(0);
            }
            self.read_buf.extend_from_slice(&tmp[..n]);
        }
    }

    /// Async write all bytes from buffer.
    pub async fn write_all(&mut self, buf: &[u8]) -> crate::Result<()> {
        let fd = self.tcp.raw_handle();
        let chunk_size = 16384;
        let mut offset = 0;
        while offset < buf.len() {
            let end = core::cmp::min(offset + chunk_size, buf.len());
            let mut encrypted = Vec::new();
            self.record
                .write_encrypted(codec::APPLICATION_DATA, &buf[offset..end], &mut encrypted);
            async_write_all(&mut self.tcp, fd, &encrypted).await?;
            offset = end;
        }
        Ok(())
    }
}

impl Drop for AsyncTlsStream {
    fn drop(&mut self) {
        // Best-effort close_notify via std::io::Write (portable, no inline asm)
        let mut alert_record = Vec::new();
        self.record
            .write_encrypted(ALERT, &[1, 0], &mut alert_record);
        self.tcp.inner_mut().set_nonblocking(false).ok();
        let stream = self.tcp.inner_mut();
        let _ = std::io::Write::write_all(stream, &alert_record);
        let _ = std::io::Write::flush(stream);
    }
}

// ── Async I/O helpers ──────────────────────────────────────────────

async fn async_read(
    tcp: &mut AsyncTcpStream,
    _handle: crate::core::platform::RawHandle,
    buf: &mut [u8],
) -> crate::Result<usize> {
    let n = tcp.read(buf).await?;
    Ok(n)
}

async fn async_write_all(
    tcp: &mut AsyncTcpStream,
    _handle: crate::core::platform::RawHandle,
    buf: &[u8],
) -> crate::Result<()> {
    tcp.write_all(buf).await
}
