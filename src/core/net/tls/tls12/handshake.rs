// TLS 1.2 client handshake state machine
//
// Drives the TLS 1.2 handshake after ServerHello:
//   Certificate → ServerKeyExchange → ServerHelloDone
//     → (send: ClientKeyExchange + CCS + Finished)
//     → ServerChangeCipherSpec → ServerFinished → Complete

use crate::core::crypto::sha256::Sha256;
use crate::core::net::tls::codec::{self, HandshakeMessage, encode_client_key_exchange, encode_change_cipher_spec};
use crate::core::net::tls::p256::Point;
use crate::core::net::tls::tls12::key_schedule::{
    derive_key_block, finished_verify_data, master_secret,
};
use crate::core::net::tls::tls12::record::Tls12RecordLayer;

// ── State ──────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Clone)]
pub enum State {
    ExpectCertificate,
    ExpectServerKeyExchange,
    ExpectServerHelloDone,
    ExpectServerChangeCipherSpec,
    ExpectServerFinished,
    Complete,
}

// ── Handshake result ───────────────────────────────────────────────

pub enum Tls12HandshakeResult {
    Continue,
    /// Raw bytes to write to the TCP stream (ClientKeyExchange record + CCS record + encrypted Finished record)
    SendToServer(Vec<u8>),
    Complete,
}

// ── Handshake ──────────────────────────────────────────────────────

pub struct Tls12Handshake {
    state: State,
    transcript: Sha256,
    client_random: [u8; 32],
    server_random: [u8; 32],
    cipher_suite: u16,
    p256_secret: [u8; 32],
    p256_public: [u8; 65],
    master_secret: Option<[u8; 48]>,
}

impl Tls12Handshake {
    /// `transcript` must already contain ClientHello + ServerHello bytes.
    pub fn new(
        transcript: Sha256,
        client_random: &[u8; 32],
        server_random: &[u8; 32],
        cipher_suite: u16,
    ) -> crate::Result<Self> {
        let mut secret = [0u8; 32];
        crate::core::crypto::getrandom(&mut secret)?;
        let public_point = Point::generator().scalar_mul(&secret);
        let p256_public = public_point
            .to_uncompressed()
            .ok_or_else(|| crate::Error::Tls("P-256 keygen produced infinity".into()))?;
        Ok(Self {
            state: State::ExpectCertificate,
            transcript,
            client_random: *client_random,
            server_random: *server_random,
            cipher_suite,
            p256_secret: secret,
            p256_public,
            master_secret: None,
        })
    }

    /// Expose current state for routing decisions in `connect_tls12`.
    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn handle_message(
        &mut self,
        msg_bytes: &[u8],
        record: &mut Tls12RecordLayer,
    ) -> crate::Result<Tls12HandshakeResult> {
        if msg_bytes.is_empty() {
            return Err(crate::Error::Tls("empty TLS 1.2 handshake message".into()));
        }
        // TLS 1.2 Certificate has no type byte (body starts with certificate_list_len).
        // We don't verify certs, so treat the raw bytes as a valid Certificate.
        let msg = if self.state == State::ExpectCertificate {
            HandshakeMessage::Certificate(Vec::new())
        } else {
            codec::decode_handshake(msg_bytes)?
        };

        match (&self.state, msg) {
            // ── Certificate ────────────────────────────────────────
            (State::ExpectCertificate, HandshakeMessage::Certificate(_certs)) => {
                self.transcript.update(msg_bytes);
                self.state = State::ExpectServerKeyExchange;
                Ok(Tls12HandshakeResult::Continue)
            }

            // ── ServerKeyExchange ──────────────────────────────────
            (State::ExpectServerKeyExchange, HandshakeMessage::ServerKeyExchange(ske)) => {
                self.transcript.update(msg_bytes);
                if ske.named_curve != 0x0017 {
                    return Err(crate::Error::Tls(format!(
                        "unsupported TLS 1.2 named curve: 0x{:04x}", ske.named_curve
                    )));
                }
                if ske.public_key.len() != 65 {
                    return Err(crate::Error::Tls("SKE public key is not 65 bytes".into()));
                }
                let mut pk_bytes = [0u8; 65];
                pk_bytes.copy_from_slice(&ske.public_key);
                let server_point = Point::from_uncompressed(&pk_bytes)?;
                let shared_point = server_point.scalar_mul(&self.p256_secret);
                let shared_x = shared_point
                    .affine_x_bytes()
                    .ok_or_else(|| crate::Error::Tls("ECDH produced infinity".into()))?;
                let ms = master_secret(&shared_x, &self.client_random, &self.server_random);
                self.master_secret = Some(ms);
                self.state = State::ExpectServerHelloDone;
                Ok(Tls12HandshakeResult::Continue)
            }

            // ── ServerHelloDone ────────────────────────────────────
            (State::ExpectServerHelloDone, HandshakeMessage::ServerHelloDone) => {
                self.transcript.update(msg_bytes);
                let ms = self.master_secret.ok_or_else(|| {
                    crate::Error::Tls("master_secret missing at ServerHelloDone".into())
                })?;
                let kb = derive_key_block(&ms, &self.server_random, &self.client_random);

                // Build ClientKeyExchange message bytes
                let mut cke_msg = Vec::new();
                encode_client_key_exchange(&self.p256_public, &mut cke_msg);
                self.transcript.update(&cke_msg);

                // Install client-side keys
                record.install_client_keys(&kb);

                // CKE as plaintext record
                let mut to_send = Vec::new();
                record.write_plaintext(codec::HANDSHAKE, &cke_msg, &mut to_send);

                // ChangeCipherSpec (plaintext full record)
                encode_change_cipher_spec(&mut to_send);

                // Build and encrypt client Finished
                let transcript_hash = self.transcript.clone().finish();
                let verify_data = finished_verify_data(&ms, b"client finished", &transcript_hash);
                let mut finished_msg = Vec::new();
                finished_msg.push(codec::FINISHED);
                finished_msg.push(0);
                finished_msg.push(0);
                finished_msg.push(12);
                finished_msg.extend_from_slice(&verify_data);
                self.transcript.update(&finished_msg);

                record.write_encrypted(codec::HANDSHAKE, &finished_msg, &mut to_send)?;

                self.state = State::ExpectServerChangeCipherSpec;
                Ok(Tls12HandshakeResult::SendToServer(to_send))
            }

            // ── Finished ───────────────────────────────────────────
            (State::ExpectServerFinished, HandshakeMessage::Finished { verify_data }) => {
                let ms = self.master_secret.ok_or_else(|| {
                    crate::Error::Tls("master_secret missing at server Finished".into())
                })?;
                if verify_data.len() < 12 {
                    return Err(crate::Error::Tls("TLS 1.2 Finished too short".into()));
                }
                let transcript_hash = self.transcript.clone().finish();
                let expected = finished_verify_data(&ms, b"server finished", &transcript_hash);
                let mut diff = 0u8;
                for i in 0..12 {
                    diff |= verify_data[i] ^ expected[i];
                }
                if diff != 0 {
                    return Err(crate::Error::Tls("TLS 1.2 server Finished verify failed".into()));
                }
                self.state = State::Complete;
                Ok(Tls12HandshakeResult::Complete)
            }

            // ── Unexpected state/message ───────────────────────────
            (state, _) => Err(crate::Error::Tls(format!(
                "TLS 1.2 unexpected message in state {:?}", state
            ))),
        }
    }

    /// Called when server ChangeCipherSpec record is received.
    pub fn handle_server_ccs(&mut self) -> crate::Result<()> {
        if self.state != State::ExpectServerChangeCipherSpec {
            return Err(crate::Error::Tls("unexpected server CCS".into()));
        }
        self.state = State::ExpectServerFinished;
        Ok(())
    }

    /// Expose cipher suite (for use by TlsStream).
    pub fn cipher_suite(&self) -> u16 {
        self.cipher_suite
    }
}
