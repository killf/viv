// TLS 1.3 client handshake state machine
//
// Drives the handshake from ClientHello through to application keys.
// The caller (TlsStream) reads records and feeds handshake messages
// to `handle_message`.

use crate::core::net::tls::codec::{self, HandshakeMessage};
use crate::core::crypto::sha256::{Sha256, hmac_sha256};
use crate::core::crypto::x25519;
use super::key_schedule::KeySchedule;
use super::record::RecordLayer;

// ── State ──────────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
enum State {
    ExpectServerHello,
    ExpectEncryptedExtensions,
    ExpectCertificate,
    ExpectCertificateVerify,
    ExpectFinished,
    Complete,
}

// ── Handshake result ───────────────────────────────────────────────

pub enum HandshakeResult {
    Continue,
    Complete,
    NegotiatedTls12 { server_random: [u8; 32], cipher_suite: u16 },
}

// ── Handshake ──────────────────────────────────────────────────────

pub struct Handshake {
    state: State,
    pub transcript: Sha256,
    pub key_schedule: KeySchedule,
    x25519_secret: [u8; 32],
    x25519_public: [u8; 32],
    server_name: String,
    random: [u8; 32],
    session_id: [u8; 32],
    /// Transcript hash after server Finished (CH..SF).
    /// Used for application traffic key derivation per RFC 8446 section 7.1.
    server_finished_hash: Option<[u8; 32]>,
}

impl Handshake {
    /// Create a new handshake. Generates X25519 keypair and random values.
    pub fn new(server_name: &str) -> crate::Result<Self> {
        let (secret, public) = x25519::keypair()?;

        let mut random = [0u8; 32];
        crate::core::crypto::getrandom(&mut random)?;

        let mut session_id = [0u8; 32];
        crate::core::crypto::getrandom(&mut session_id)?;

        Ok(Self {
            state: State::ExpectServerHello,
            transcript: Sha256::new(),
            key_schedule: KeySchedule::new(),
            x25519_secret: secret,
            x25519_public: public,
            server_name: server_name.to_string(),
            random,
            session_id,
            server_finished_hash: None,
        })
    }

    /// Encode the ClientHello message and add it to the transcript.
    /// Returns the raw handshake message bytes (to be sent as a plaintext record).
    pub fn encode_client_hello(&mut self) -> crate::Result<Vec<u8>> {
        let mut msg = Vec::new();
        codec::encode_client_hello(
            &self.random,
            &self.session_id,
            &self.server_name,
            &self.x25519_public,
            &mut msg,
        );
        self.transcript.update(&msg);
        Ok(msg)
    }

    /// Process a single handshake message. `msg_bytes` is the raw bytes
    /// starting from the handshake header (type + 3-byte length).
    ///
    /// For the Finished message, transcript handling is special:
    /// verify using hash BEFORE adding msg_bytes, then add.
    pub fn handle_message(
        &mut self,
        msg_bytes: &[u8],
        record: &mut RecordLayer,
    ) -> crate::Result<HandshakeResult> {
        let msg = codec::decode_handshake(msg_bytes)?;

        match (&self.state, msg) {
            // ── ServerHello ────────────────────────────────────────
            (State::ExpectServerHello, HandshakeMessage::ServerHello(sh)) => {
                // Add ServerHello to transcript
                self.transcript.update(msg_bytes);

                // TLS 1.2 negotiated — signal to caller
                if sh.version == 0x0303 {
                    return Ok(HandshakeResult::NegotiatedTls12 {
                        server_random: sh.random,
                        cipher_suite: sh.cipher_suite,
                    });
                }

                // Verify cipher suite (TLS 1.3 only path from here)
                if sh.cipher_suite != 0x1301 {
                    return Err(crate::Error::Tls(format!(
                        "unsupported cipher suite: 0x{:04x}",
                        sh.cipher_suite
                    )));
                }

                // Compute shared secret
                let x25519_pub = sh.x25519_public.ok_or_else(|| {
                    crate::Error::Tls("TLS 1.3 ServerHello missing key_share".into())
                })?;
                let shared = x25519::shared_secret(&self.x25519_secret, &x25519_pub);

                // RFC 7748 §6.1: reject all-zero shared secret (low-order point)
                if shared == [0u8; 32] {
                    return Err(crate::Error::Tls(
                        "X25519 shared secret is all-zero (low-order point)".into(),
                    ));
                }

                // Derive handshake secrets
                let hello_hash = self.transcript.clone().finish();
                let (client_hs_keys, server_hs_keys) = self
                    .key_schedule
                    .derive_handshake_secrets(&shared, &hello_hash)?;

                // Install server handshake decrypter
                record.install_decrypter(server_hs_keys.key, server_hs_keys.iv);

                // Install the client encrypter so we can send Finished encrypted
                record.install_encrypter(client_hs_keys.key, client_hs_keys.iv);

                self.state = State::ExpectEncryptedExtensions;
                Ok(HandshakeResult::Continue)
            }

            // ── EncryptedExtensions ────────────────────────────────
            (State::ExpectEncryptedExtensions, HandshakeMessage::EncryptedExtensions) => {
                self.transcript.update(msg_bytes);
                self.state = State::ExpectCertificate;
                Ok(HandshakeResult::Continue)
            }

            // ── Certificate ────────────────────────────────────────
            (State::ExpectCertificate, HandshakeMessage::Certificate(_certs)) => {
                self.transcript.update(msg_bytes);
                // TODO: verify certificate chain (x509.rs stub)
                self.state = State::ExpectCertificateVerify;
                Ok(HandshakeResult::Continue)
            }

            // ── CertificateVerify ──────────────────────────────────
            (State::ExpectCertificateVerify, HandshakeMessage::CertificateVerify { .. }) => {
                self.transcript.update(msg_bytes);
                // TODO: verify signature against certificate public key
                self.state = State::ExpectFinished;
                Ok(HandshakeResult::Continue)
            }

            // ── Finished ───────────────────────────────────────────
            (State::ExpectFinished, HandshakeMessage::Finished { verify_data }) => {
                if verify_data.len() < 32 {
                    return Err(crate::Error::Tls("TLS 1.3 Finished too short".into()));
                }
                let mut vd = [0u8; 32];
                vd.copy_from_slice(&verify_data[..32]);

                // CRITICAL: snapshot transcript BEFORE adding Finished message
                let transcript_before = self.transcript.clone().finish();

                // Verify server Finished
                let server_finished_key = self.key_schedule.server_finished_key()?;
                let expected = hmac_sha256(&server_finished_key, &transcript_before);

                // Constant-time comparison
                let mut diff = 0u8;
                for i in 0..32 {
                    diff |= vd[i] ^ expected[i];
                }
                if diff != 0 {
                    return Err(crate::Error::Tls("server Finished verify failed".into()));
                }

                // Now add Finished to transcript
                self.transcript.update(msg_bytes);

                // Save the transcript hash at this point (CH..SF) for app key derivation.
                // Per RFC 8446 section 7.1, application traffic secrets use
                // Transcript-Hash(ClientHello..server Finished), NOT the hash
                // after client Finished.
                self.server_finished_hash = Some(self.transcript.clone().finish());

                self.state = State::Complete;
                Ok(HandshakeResult::Complete)
            }

            // ── Unexpected state/message ───────────────────────────
            _ => Err(crate::Error::Tls(format!(
                "unexpected handshake message in state {:?}",
                self.state
            ))),
        }
    }

    /// Encode the client Finished message. Call after handle_message
    /// returns Complete.
    pub fn encode_client_finished(&mut self) -> crate::Result<Vec<u8>> {
        // verify_data = HMAC(client_finished_key, transcript_hash)
        // At this point transcript contains CH..SF
        let transcript_hash = self.transcript.clone().finish();
        let client_finished_key = self.key_schedule.client_finished_key()?;
        let verify_data = hmac_sha256(&client_finished_key, &transcript_hash);

        let mut msg = Vec::new();
        let vd: [u8; 32] = verify_data;
        codec::encode_finished(&vd, &mut msg);

        // Add client Finished to transcript
        self.transcript.update(&msg);

        Ok(msg)
    }

    /// Derive and install application traffic keys on the record layer.
    /// Call after sending client Finished.
    ///
    /// Uses the transcript hash saved after server Finished (CH..SF),
    /// per RFC 8446 section 7.1.
    pub fn install_app_keys(&mut self, record: &mut RecordLayer) -> crate::Result<()> {
        let hash = self.server_finished_hash.ok_or_else(|| {
            crate::Error::Tls(
                "install_app_keys called before server Finished".to_string(),
            )
        })?;
        let (client_app, server_app) = self.key_schedule.derive_app_secrets(&hash)?;
        record.install_encrypter(client_app.key, client_app.iv);
        record.install_decrypter(server_app.key, server_app.iv);
        Ok(())
    }

    /// Expose client random for TLS 1.2 fallback.
    pub fn client_random(&self) -> &[u8; 32] {
        &self.random
    }
}
