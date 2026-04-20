// TLS 1.3 record layer — framing + AEAD encrypt/decrypt
//
// Handles reading and writing TLS records. Before keys are installed,
// records are plaintext. After install_encrypter/install_decrypter,
// records use AES-128-GCM with per-record nonce derivation.

use super::crypto::aes_gcm::Aes128Gcm;

// ── Record encrypter ───────────────────────────────────────────────

struct RecordEncrypter {
    cipher: Aes128Gcm,
    iv: [u8; 12],
    seq: u64,
}

impl RecordEncrypter {
    fn new(key: [u8; 16], iv: [u8; 12]) -> Self {
        Self {
            cipher: Aes128Gcm::new(&key),
            iv,
            seq: 0,
        }
    }

    /// Compute the per-record nonce: iv XOR (0x00000000 || seq_as_u64_be).
    fn nonce(&self) -> [u8; 12] {
        let mut n = self.iv;
        let seq_bytes = self.seq.to_be_bytes();
        // XOR the last 8 bytes of the 12-byte IV with seq
        for i in 0..8 {
            n[4 + i] ^= seq_bytes[i];
        }
        n
    }

    /// Encrypt payload with inner content type appended, write the
    /// complete TLS record (header + ciphertext + tag) to `out`.
    fn encrypt(&mut self, content_type: u8, payload: &[u8], out: &mut Vec<u8>) -> crate::Result<()> {
        // Inner plaintext = payload || content_type
        let mut inner = Vec::with_capacity(payload.len() + 1);
        inner.extend_from_slice(payload);
        inner.push(content_type);

        // Ciphertext length = inner.len() + 16 (GCM tag)
        let ct_len = inner.len() + 16;

        // Record header: outer type APPLICATION_DATA, version 0x0303
        let header_start = out.len();
        out.push(super::codec::APPLICATION_DATA);
        out.push(0x03);
        out.push(0x03);
        out.push((ct_len >> 8) as u8);
        out.push(ct_len as u8);

        // AAD = the 5-byte record header
        let aad: [u8; 5] = [
            out[header_start],
            out[header_start + 1],
            out[header_start + 2],
            out[header_start + 3],
            out[header_start + 4],
        ];

        let nonce = self.nonce();

        // Allocate space for ciphertext + tag
        let ct_start = out.len();
        out.resize(ct_start + ct_len, 0);
        self.cipher
            .encrypt(&nonce, &aad, &inner, &mut out[ct_start..])?;

        self.seq += 1;
        Ok(())
    }
}

// ── Record decrypter ───────────────────────────────────────────────

struct RecordDecrypter {
    cipher: Aes128Gcm,
    iv: [u8; 12],
    seq: u64,
}

impl RecordDecrypter {
    fn new(key: [u8; 16], iv: [u8; 12]) -> Self {
        Self {
            cipher: Aes128Gcm::new(&key),
            iv,
            seq: 0,
        }
    }

    fn nonce(&self) -> [u8; 12] {
        let mut n = self.iv;
        let seq_bytes = self.seq.to_be_bytes();
        for i in 0..8 {
            n[4 + i] ^= seq_bytes[i];
        }
        n
    }

    /// Decrypt a record payload. `header` is the 5-byte record header (AAD).
    /// Returns (real_content_type, plaintext).
    fn decrypt(
        &mut self,
        header: &[u8; 5],
        ciphertext_and_tag: &[u8],
    ) -> crate::Result<(u8, Vec<u8>)> {
        if ciphertext_and_tag.len() < 17 {
            // At minimum: 1 byte content type + 16 byte tag
            return Err(crate::Error::Tls("encrypted record too short".into()));
        }

        let nonce = self.nonce();

        let mut plaintext = vec![0u8; ciphertext_and_tag.len() - 16];
        self.cipher
            .decrypt(&nonce, header, ciphertext_and_tag, &mut plaintext)?;

        self.seq += 1;

        // Strip trailing zeros and find the real content type (last non-zero byte)
        let real_ct = loop {
            match plaintext.pop() {
                Some(0) => continue,
                Some(ct) => break ct,
                None => return Err(crate::Error::Tls("empty decrypted record".into())),
            }
        };

        Ok((real_ct, plaintext))
    }
}

// ── RecordLayer ────────────────────────────────────────────────────

pub struct RecordLayer {
    encrypter: Option<RecordEncrypter>,
    decrypter: Option<RecordDecrypter>,
}

impl Default for RecordLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl RecordLayer {
    pub fn new() -> Self {
        Self {
            encrypter: None,
            decrypter: None,
        }
    }

    pub fn install_encrypter(&mut self, key: [u8; 16], iv: [u8; 12]) {
        self.encrypter = Some(RecordEncrypter::new(key, iv));
    }

    pub fn install_decrypter(&mut self, key: [u8; 16], iv: [u8; 12]) {
        self.decrypter = Some(RecordDecrypter::new(key, iv));
    }

    /// Write a plaintext TLS record (no encryption).
    /// Format: content_type(1) + version(0x0301)(2) + length(2) + payload.
    pub fn write_plaintext(&self, content_type: u8, payload: &[u8], out: &mut Vec<u8>) {
        out.push(content_type);
        out.push(0x03);
        out.push(0x01);
        out.push((payload.len() >> 8) as u8);
        out.push(payload.len() as u8);
        out.extend_from_slice(payload);
    }

    /// Write an encrypted TLS record (AEAD). Returns an error if no encrypter is installed.
    pub fn write_encrypted(
        &mut self,
        content_type: u8,
        payload: &[u8],
        out: &mut Vec<u8>,
    ) -> crate::Result<()> {
        let enc = self.encrypter.as_mut().ok_or_else(|| {
            crate::Error::Tls("write_encrypted called before encrypter installed".to_string())
        })?;
        enc.encrypt(content_type, payload, out)
    }

    /// Read one TLS record from `data`.
    ///
    /// Returns (real_content_type, payload, bytes_consumed).
    /// - If decrypter is installed and record type is APPLICATION_DATA,
    ///   the payload is decrypted and the inner content type is returned.
    /// - Otherwise, payload is returned as-is.
    pub fn read_record(&mut self, data: &[u8]) -> crate::Result<(u8, Vec<u8>, usize)> {
        if data.len() < 5 {
            return Err(crate::Error::Tls("record header too short".into()));
        }

        let content_type = data[0];
        // data[1..3] = version (ignore)
        let length = ((data[3] as usize) << 8) | (data[4] as usize);

        if data.len() < 5 + length {
            return Err(crate::Error::Tls("record body truncated".into()));
        }

        let consumed = 5 + length;
        let payload = &data[5..consumed];

        if content_type == super::codec::APPLICATION_DATA
            && let Some(dec) = &mut self.decrypter
        {
            let header: [u8; 5] = [data[0], data[1], data[2], data[3], data[4]];
            let (real_ct, plaintext) = dec.decrypt(&header, payload)?;
            return Ok((real_ct, plaintext, consumed));
        }

        Ok((content_type, payload.to_vec(), consumed))
    }
}
