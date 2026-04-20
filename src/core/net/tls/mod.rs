// TLS 1.3 pure-Rust implementation
//
// Provides async-only TlsStream that implements the full TLS 1.3 handshake
// and record layer using only Rust code with zero external dependencies.

pub mod codec;
pub mod ecdsa;
pub mod p256;
pub mod rsa;
pub mod tls12;
pub mod tls13;
pub mod x509;

// Keep existing paths valid: viv::core::net::tls::{handshake, key_schedule, record}
pub use tls13::handshake;
pub use tls13::key_schedule;
pub use tls13::record;

use std::io::{Read, Write};

use codec::{ALERT, APPLICATION_DATA, CHANGE_CIPHER_SPEC, HANDSHAKE};
use tls12::record::Tls12RecordLayer;
use tls13::handshake::{Handshake, HandshakeResult};
use tls13::record::RecordLayer;

// ── RecordLayerVariant ────────────────────────────────────────────
//
// Runtime-dispatched record layer that delegates to either the
// TLS 1.3 or TLS 1.2 implementation depending on what was negotiated.

enum RecordLayerVariant {
    Tls13(RecordLayer),
    Tls12(Tls12RecordLayer),
}

impl RecordLayerVariant {
    fn write_encrypted(
        &mut self,
        ct: u8,
        payload: &[u8],
        out: &mut Vec<u8>,
    ) -> crate::Result<()> {
        match self {
            RecordLayerVariant::Tls13(r) => r.write_encrypted(ct, payload, out),
            RecordLayerVariant::Tls12(r) => r.write_encrypted(ct, payload, out),
        }
    }

    fn read_record(&mut self, data: &[u8]) -> crate::Result<(u8, Vec<u8>, usize)> {
        match self {
            RecordLayerVariant::Tls13(r) => r.read_record(data),
            RecordLayerVariant::Tls12(r) => r.read_record(data),
        }
    }
}

// ── TlsStream (async, TLS 1.3 + TLS 1.2) ───────────────────────────

use super::async_tcp::AsyncTcpStream;

pub struct TlsStream {
    tcp: AsyncTcpStream,
    record: RecordLayerVariant,
    read_buf: Vec<u8>,
    plaintext_buf: Vec<u8>,
}

unsafe impl Send for TlsStream {}

impl TlsStream {
    /// Connect to `host:port` over TLS with async I/O.
    /// Negotiates TLS 1.3 preferred, falls back to TLS 1.2 automatically.
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
        let mut tls12_triggered = false;
        let mut tls12_server_random = [0u8; 32];
        let mut tls12_cipher_suite = 0u16;
        let mut tls12_transcript_sha: Option<crate::core::crypto::sha256::Sha256> = None;

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

                            match result {
                                HandshakeResult::Complete => break 'outer,
                                HandshakeResult::NegotiatedTls12 {
                                    server_random,
                                    cipher_suite,
                                    transcript,
                                } => {
                                    tls12_triggered = true;
                                    tls12_server_random = server_random;
                                    tls12_cipher_suite = cipher_suite;
                                    tls12_transcript_sha = Some(transcript);
                                    break 'outer;
                                }
                                HandshakeResult::Continue => {}
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

            let n = async_read(&mut tcp, fd, &mut tmp).await?;
            if n == 0 {
                return Err(crate::Error::Tls(
                    "connection closed during handshake".into(),
                ));
            }
            read_buf.extend_from_slice(&tmp[..n]);
        }

        if tls12_triggered {
            // TLS 1.2: use blocking I/O for the handshake phase
            let transcript = tls12_transcript_sha.ok_or_else(|| {
                crate::Error::Tls("TLS 1.2 triggered but transcript missing".into())
            })?;
            let tcp_stream = tcp.into_inner();
            tcp_stream.set_nonblocking(false)?;
            return Self::connect_tls12(
                tcp_stream,
                hs.client_random(),
                &tls12_server_random,
                tls12_cipher_suite,
                transcript,
                read_buf,
            );
        }

        // TLS 1.3 path
        // 3. Send CCS
        let mut ccs = Vec::new();
        codec::encode_change_cipher_spec(&mut ccs);
        async_write_all(&mut tcp, fd, &ccs).await?;

        // 4. Send client Finished
        let finished_msg = hs.encode_client_finished()?;
        let mut finished_record = Vec::new();
        record.write_encrypted(HANDSHAKE, &finished_msg, &mut finished_record)?;
        async_write_all(&mut tcp, fd, &finished_record).await?;

        // 5. Install app keys
        hs.install_app_keys(&mut record)?;

        Ok(TlsStream {
            tcp,
            record: RecordLayerVariant::Tls13(record),
            read_buf,
            plaintext_buf: Vec::new(),
        })
    }

    /// TLS 1.2 handshake fallback. Uses blocking I/O on the raw TcpStream.
    fn connect_tls12(
        mut tcp_stream: std::net::TcpStream,
        client_random: &[u8; 32],
        server_random: &[u8; 32],
        cipher_suite: u16,
        transcript: crate::core::crypto::sha256::Sha256,
        mut read_buf: Vec<u8>,
    ) -> crate::Result<Self> {
        use tls12::handshake::{Tls12Handshake, Tls12HandshakeResult};

        let mut hs =
            Tls12Handshake::new(transcript, client_random, server_random, cipher_suite)?;
        let mut tls12_record = Tls12RecordLayer::new();
        let mut tmp = [0u8; 8192];

        loop {
            loop {
                if read_buf.len() < 5 {
                    break;
                }
                let rec_len = ((read_buf[3] as usize) << 8) | (read_buf[4] as usize);
                if read_buf.len() < 5 + rec_len {
                    break;
                }

                let (ct, payload, consumed) = tls12_record.read_record(&read_buf)?;
                read_buf.drain(..consumed);

                match ct {
                    CHANGE_CIPHER_SPEC => {
                        hs.handle_server_ccs()?;
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
                    HANDSHAKE | APPLICATION_DATA => {
                        let mut pos = 0;
                        while pos < payload.len() {
                            if pos + 4 > payload.len() {
                                return Err(crate::Error::Tls(
                                    "truncated TLS 1.2 handshake".into(),
                                ));
                            }
                            let _msg_type = payload[pos];
                            let msg_len = ((payload[pos + 1] as usize) << 16)
                                | ((payload[pos + 2] as usize) << 8)
                                | payload[pos + 3] as usize;
                            let msg_end = pos + 4 + msg_len;
                            if msg_end > payload.len() {
                                return Err(crate::Error::Tls(
                                    "TLS 1.2 HS msg exceeds record".into(),
                                ));
                            }
                            let msg_bytes = &payload[pos..msg_end];
                            let result = hs.handle_message(msg_bytes, &mut tls12_record)?;
                            pos = msg_end;

                            match result {
                                Tls12HandshakeResult::Continue => {}
                                Tls12HandshakeResult::SendToServer(bytes) => {
                                    tcp_stream.write_all(&bytes).map_err(crate::Error::Io)?;
                                }
                                Tls12HandshakeResult::Complete => {
                                    let tcp = AsyncTcpStream::from_std(tcp_stream)?;
                                    return Ok(TlsStream {
                                        tcp,
                                        record: RecordLayerVariant::Tls12(tls12_record),
                                        read_buf,
                                        plaintext_buf: Vec::new(),
                                    });
                                }
                            }
                        }
                    }
                    _ => {
                        return Err(crate::Error::Tls(format!(
                            "unexpected TLS 1.2 record type: 0x{:02x}",
                            ct
                        )));
                    }
                }
            }

            let n = tcp_stream.read(&mut tmp).map_err(crate::Error::Io)?;
            if n == 0 {
                return Err(crate::Error::Tls(
                    "connection closed during TLS 1.2 handshake".into(),
                ));
            }
            read_buf.extend_from_slice(&tmp[..n]);
        }
    }

    pub fn raw_handle(&self) -> crate::core::platform::RawHandle {
        self.tcp.raw_handle()
    }

    /// Async read into buffer. Returns number of bytes read (0 = EOF).
    pub async fn read(&mut self, buf: &mut [u8]) -> crate::Result<usize> {
        if !self.plaintext_buf.is_empty() {
            let n = core::cmp::min(buf.len(), self.plaintext_buf.len());
            buf[..n].copy_from_slice(&self.plaintext_buf[..n]);
            self.plaintext_buf.drain(..n);
            return Ok(n);
        }

        let fd = self.tcp.raw_handle();
        let mut tmp = [0u8; 8192];

        loop {
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
                        HANDSHAKE => continue,
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
                .write_encrypted(codec::APPLICATION_DATA, &buf[offset..end], &mut encrypted)?;
            async_write_all(&mut self.tcp, fd, &encrypted).await?;
            offset = end;
        }
        Ok(())
    }
}

impl Drop for TlsStream {
    fn drop(&mut self) {
        let mut alert_record = Vec::new();
        if self
            .record
            .write_encrypted(ALERT, &[1, 0], &mut alert_record)
            .is_ok()
        {
            // Use blocking I/O to send close_notify
            self.tcp.inner_mut().set_nonblocking(false).ok();
            let stream = self.tcp.inner_mut();
            let _ = std::io::Write::write_all(stream, &alert_record);
            let _ = std::io::Write::flush(stream);
            self.tcp.inner_mut().set_nonblocking(true).ok();
        }
    }
}

// ── Async I/O helpers ──────────────────────────────────────────────

async fn async_read(
    tcp: &mut AsyncTcpStream,
    _handle: crate::core::platform::RawHandle,
    buf: &mut [u8],
) -> crate::Result<usize> {
    tcp.read(buf).await
}

async fn async_write_all(
    tcp: &mut AsyncTcpStream,
    _handle: crate::core::platform::RawHandle,
    buf: &[u8],
) -> crate::Result<()> {
    tcp.write_all(buf).await
}
