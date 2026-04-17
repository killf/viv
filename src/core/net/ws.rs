use crate::error::Error;

/// WebSocket frame opcodes (RFC 6455 Section 5.2).
#[derive(Debug, Clone, PartialEq)]
pub enum WsOpcode {
    Text,   // 0x1
    Binary, // 0x2
    Close,  // 0x8
    Ping,   // 0x9
    Pong,   // 0xA
}

impl WsOpcode {
    fn to_byte(&self) -> u8 {
        match self {
            WsOpcode::Text => 0x1,
            WsOpcode::Binary => 0x2,
            WsOpcode::Close => 0x8,
            WsOpcode::Ping => 0x9,
            WsOpcode::Pong => 0xA,
        }
    }

    fn from_byte(b: u8) -> crate::Result<Self> {
        match b {
            0x1 => Ok(WsOpcode::Text),
            0x2 => Ok(WsOpcode::Binary),
            0x8 => Ok(WsOpcode::Close),
            0x9 => Ok(WsOpcode::Ping),
            0xA => Ok(WsOpcode::Pong),
            _ => Err(Error::Http(format!(
                "unknown WebSocket opcode: 0x{:02X}",
                b
            ))),
        }
    }
}

/// A WebSocket frame.
pub struct WsFrame {
    pub opcode: WsOpcode,
    pub payload: Vec<u8>,
}

impl WsFrame {
    /// Create a text frame.
    pub fn text(s: &str) -> Self {
        WsFrame {
            opcode: WsOpcode::Text,
            payload: s.as_bytes().to_vec(),
        }
    }

    /// Create a close frame with empty payload.
    pub fn close() -> Self {
        WsFrame {
            opcode: WsOpcode::Close,
            payload: Vec::new(),
        }
    }

    /// Create a pong frame echoing the given data.
    pub fn pong(data: &[u8]) -> Self {
        WsFrame {
            opcode: WsOpcode::Pong,
            payload: data.to_vec(),
        }
    }

    /// Encode a client frame with masking (RFC 6455 Section 5.3).
    ///
    /// - FIN bit always set
    /// - MASK bit always set (client-to-server requirement)
    /// - 4-byte mask key derived from timestamp
    pub fn encode(&self) -> Vec<u8> {
        let len = self.payload.len();
        let mut buf = Vec::with_capacity(2 + 8 + 4 + len);

        // Byte 0: FIN (0x80) | opcode
        buf.push(0x80 | self.opcode.to_byte());

        // Byte 1+: MASK (0x80) | payload length encoding
        if len < 126 {
            buf.push(0x80 | len as u8);
        } else if len <= 0xFFFF {
            buf.push(0x80 | 126);
            buf.extend_from_slice(&(len as u16).to_be_bytes());
        } else {
            buf.push(0x80 | 127);
            buf.extend_from_slice(&(len as u64).to_be_bytes());
        }

        // 4-byte mask key (pseudo-random from timestamp)
        let mask = generate_mask_key();
        buf.extend_from_slice(&mask);

        // Masked payload
        for (i, &b) in self.payload.iter().enumerate() {
            buf.push(b ^ mask[i % 4]);
        }

        buf
    }

    /// Decode a server frame (typically unmasked, but masked frames are also supported).
    ///
    /// Returns `Ok(None)` if the data is incomplete.
    /// Returns `Ok(Some((frame, consumed_bytes)))` on success.
    pub fn decode(data: &[u8]) -> crate::Result<Option<(Self, usize)>> {
        // Need at least 2 bytes for the header
        if data.len() < 2 {
            return Ok(None);
        }

        let byte0 = data[0];
        let byte1 = data[1];

        // Parse opcode from lower 4 bits of byte 0 (ignore FIN/RSV for now)
        let opcode = WsOpcode::from_byte(byte0 & 0x0F)?;
        let masked = (byte1 & 0x80) != 0;
        let len_field = (byte1 & 0x7F) as usize;

        let mut offset = 2;

        // Determine actual payload length
        let payload_len = if len_field < 126 {
            len_field
        } else if len_field == 126 {
            if data.len() < offset + 2 {
                return Ok(None);
            }
            let len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
            offset += 2;
            len
        } else {
            // len_field == 127
            if data.len() < offset + 8 {
                return Ok(None);
            }
            let len = u64::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]) as usize;
            offset += 8;
            len
        };

        // Read mask key if present
        let mask_key = if masked {
            if data.len() < offset + 4 {
                return Ok(None);
            }
            let key = [
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ];
            offset += 4;
            Some(key)
        } else {
            None
        };

        // Check we have enough data for the payload
        if data.len() < offset + payload_len {
            return Ok(None);
        }

        // Extract and unmask payload
        let mut payload = data[offset..offset + payload_len].to_vec();
        if let Some(mask) = mask_key {
            for (i, b) in payload.iter_mut().enumerate() {
                *b ^= mask[i % 4];
            }
        }

        let consumed = offset + payload_len;
        Ok(Some((WsFrame { opcode, payload }, consumed)))
    }
}

/// Build an HTTP/1.1 WebSocket upgrade request (RFC 6455 Section 4.1).
pub fn build_upgrade_request(host: &str, path: &str) -> Vec<u8> {
    let key = generate_ws_key();
    let mut req = String::new();
    req.push_str("GET ");
    req.push_str(path);
    req.push_str(" HTTP/1.1\r\n");
    req.push_str("Host: ");
    req.push_str(host);
    req.push_str("\r\n");
    req.push_str("Upgrade: websocket\r\n");
    req.push_str("Connection: Upgrade\r\n");
    req.push_str("Sec-WebSocket-Version: 13\r\n");
    req.push_str("Sec-WebSocket-Key: ");
    req.push_str(&key);
    req.push_str("\r\n");
    req.push_str("\r\n");
    req.into_bytes()
}

/// Base64 encode (RFC 4648, standard alphabet, with padding).
pub fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);

    let chunks = data.chunks(3);
    for chunk in chunks {
        match chunk.len() {
            3 => {
                let n = (chunk[0] as u32) << 16 | (chunk[1] as u32) << 8 | chunk[2] as u32;
                out.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
                out.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
                out.push(ALPHABET[((n >> 6) & 0x3F) as usize] as char);
                out.push(ALPHABET[(n & 0x3F) as usize] as char);
            }
            2 => {
                let n = (chunk[0] as u32) << 16 | (chunk[1] as u32) << 8;
                out.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
                out.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
                out.push(ALPHABET[((n >> 6) & 0x3F) as usize] as char);
                out.push('=');
            }
            1 => {
                let n = (chunk[0] as u32) << 16;
                out.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
                out.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
                out.push('=');
                out.push('=');
            }
            _ => {}
        }
    }

    out
}

// ---- helpers ----------------------------------------------------------------

/// Generate a 4-byte mask key from the current timestamp.
fn generate_mask_key() -> [u8; 4] {
    // Use timestamp nanoseconds for pseudo-randomness (no external deps).
    let mut ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    let mut key = [0u8; 4];
    for b in &mut key {
        *b = (ts & 0xFF) as u8;
        // Simple LCG-like mixing to avoid identical bytes
        ts = ts.wrapping_mul(6364136223846793005).wrapping_add(1);
    }
    key
}

/// Generate a random 16-byte Sec-WebSocket-Key, base64-encoded.
fn generate_ws_key() -> String {
    let mut ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    let mut bytes = [0u8; 16];
    for b in &mut bytes {
        ts = ts
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *b = ((ts >> 33) & 0xFF) as u8;
    }
    base64_encode(&bytes)
}
