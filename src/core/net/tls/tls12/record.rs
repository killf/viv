// TLS 1.2 record layer — framing + AEAD encrypt/decrypt with explicit IV
//
// Handles reading and writing TLS records. Before keys are installed,
// records are plaintext. After install_client_keys/install_server_keys,
// records use AES-128-GCM with explicit IV (8 bytes) per RFC 5246.

use crate::core::crypto::aes_gcm::Aes128Gcm;
use crate::core::net::tls::tls12::key_schedule::KeyBlock;

// ── Record encrypter ───────────────────────────────────────────────

struct Encrypter {
    cipher: Aes128Gcm,
    implicit_iv: [u8; 4],
    seq: u64,
}

impl Encrypter {
    fn new(key: [u8; 16], implicit_iv: [u8; 4]) -> Self {
        Self {
            cipher: Aes128Gcm::new(&key),
            implicit_iv,
            seq: 0,
        }
    }

    /// Encrypt plaintext with explicit IV and tag.
    /// Record format: header(5) + explicit_iv(8) + ciphertext + tag(16)
    fn encrypt(&mut self, content_type: u8, payload: &[u8], out: &mut Vec<u8>) -> crate::Result<()> {
        let explicit_iv = self.seq.to_be_bytes();
        let mut nonce = [0u8; 12];
        nonce[..4].copy_from_slice(&self.implicit_iv);
        nonce[4..].copy_from_slice(&explicit_iv);

        // AAD = seq(8) | type(1) | version(2=0x0303) | plaintext_len(2)
        let plain_len = payload.len() as u16;
        let mut aad = [0u8; 13];
        aad[..8].copy_from_slice(&explicit_iv);
        aad[8] = content_type;
        aad[9] = 0x03;
        aad[10] = 0x03;
        aad[11] = (plain_len >> 8) as u8;
        aad[12] = plain_len as u8;

        // Record: header(5) + explicit_iv(8) + ciphertext(n) + tag(16)
        let ct_len = 8 + payload.len() + 16;
        out.push(content_type);
        out.push(0x03);
        out.push(0x03);
        out.push((ct_len >> 8) as u8);
        out.push(ct_len as u8);
        out.extend_from_slice(&explicit_iv);

        let ct_start = out.len();
        out.resize(ct_start + payload.len() + 16, 0);
        self.cipher.encrypt(&nonce, &aad, payload, &mut out[ct_start..])?;

        self.seq += 1;
        Ok(())
    }
}

// ── Record decrypter ───────────────────────────────────────────────

struct Decrypter {
    cipher: Aes128Gcm,
    implicit_iv: [u8; 4],
    seq: u64,
}

impl Decrypter {
    fn new(key: [u8; 16], implicit_iv: [u8; 4]) -> Self {
        Self {
            cipher: Aes128Gcm::new(&key),
            implicit_iv,
            seq: 0,
        }
    }

    /// Decrypt a record payload (explicit_iv(8) + ciphertext + tag(16)).
    fn decrypt(&mut self, content_type: u8, payload: &[u8]) -> crate::Result<Vec<u8>> {
        if payload.len() < 8 + 16 {
            return Err(crate::Error::Tls("TLS 1.2 record too short to decrypt".to_string()));
        }
        let mut explicit_iv = [0u8; 8];
        explicit_iv.copy_from_slice(&payload[..8]);
        let ciphertext_and_tag = &payload[8..];

        let mut nonce = [0u8; 12];
        nonce[..4].copy_from_slice(&self.implicit_iv);
        nonce[4..].copy_from_slice(&explicit_iv);

        let plain_len = (ciphertext_and_tag.len() - 16) as u16;
        let mut aad = [0u8; 13];
        aad[..8].copy_from_slice(&self.seq.to_be_bytes());
        aad[8] = content_type;
        aad[9] = 0x03;
        aad[10] = 0x03;
        aad[11] = (plain_len >> 8) as u8;
        aad[12] = plain_len as u8;

        let mut plaintext = vec![0u8; ciphertext_and_tag.len() - 16];
        self.cipher.decrypt(&nonce, &aad, ciphertext_and_tag, &mut plaintext)?;

        self.seq += 1;
        Ok(plaintext)
    }
}

// ── Tls12RecordLayer ───────────────────────────────────────────────

pub struct Tls12RecordLayer {
    encrypter: Option<Encrypter>,
    decrypter: Option<Decrypter>,
}

impl Default for Tls12RecordLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Tls12RecordLayer {
    pub fn new() -> Self {
        Self {
            encrypter: None,
            decrypter: None,
        }
    }

    /// Install client-side keys (client encrypts, server decrypts from client perspective).
    pub fn install_client_keys(&mut self, kb: &KeyBlock) {
        self.encrypter = Some(Encrypter::new(kb.client_write_key, kb.client_write_iv));
        self.decrypter = Some(Decrypter::new(kb.server_write_key, kb.server_write_iv));
    }

    /// Install server-side keys (for testing: server encrypts, client decrypts).
    pub fn install_server_keys(&mut self, kb: &KeyBlock) {
        self.encrypter = Some(Encrypter::new(kb.server_write_key, kb.server_write_iv));
        self.decrypter = Some(Decrypter::new(kb.client_write_key, kb.client_write_iv));
    }

    /// Write a plaintext TLS record (no encryption).
    /// Format: content_type(1) + version(0x0303)(2) + length(2) + payload.
    pub fn write_plaintext(&self, content_type: u8, payload: &[u8], out: &mut Vec<u8>) {
        out.push(content_type);
        out.push(0x03);
        out.push(0x03);
        out.push((payload.len() >> 8) as u8);
        out.push(payload.len() as u8);
        out.extend_from_slice(payload);
    }

    /// Write an encrypted TLS record (AEAD). Returns an error if no encrypter is installed.
    pub fn write_encrypted(&mut self, content_type: u8, payload: &[u8], out: &mut Vec<u8>) -> crate::Result<()> {
        let enc = self.encrypter.as_mut().ok_or_else(|| {
            crate::Error::Tls("write_encrypted called before keys installed".to_string())
        })?;
        enc.encrypt(content_type, payload, out)
    }

    /// Read one TLS record from `data`. Returns (content_type, plaintext, bytes_consumed).
    /// If decrypter is installed and content_type is APPLICATION_DATA (0x17),
    /// the payload is decrypted. Otherwise, payload is returned as-is.
    pub fn read_record(&mut self, data: &[u8]) -> crate::Result<(u8, Vec<u8>, usize)> {
        if data.len() < 5 {
            return Err(crate::Error::Tls("record too short".to_string()));
        }
        let content_type = data[0];
        let length = ((data[3] as usize) << 8) | data[4] as usize;
        if data.len() < 5 + length {
            return Err(crate::Error::Tls("record body truncated".to_string()));
        }
        let consumed = 5 + length;
        let payload = &data[5..consumed];

        // APPLICATION_DATA (0x17) and HANDSHAKE (0x16) get decrypted if keys are installed.
        // After server ChangeCipherSpec, the encrypted Finished record uses content type
        // HANDSHAKE (0x16), so both types must be decrypted here.
        if content_type == 0x17 || content_type == 0x16 {
            if let Some(dec) = &mut self.decrypter {
                let plaintext = dec.decrypt(content_type, payload)?;
                return Ok((content_type, plaintext, consumed));
            }
        }
        Ok((content_type, payload.to_vec(), consumed))
    }
}
