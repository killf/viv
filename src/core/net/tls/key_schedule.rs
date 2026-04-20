// TLS 1.3 Key Schedule (RFC 8446 section 7.1)
//
// Hardcoded for SHA-256 + AES-128-GCM (TLS_AES_128_GCM_SHA256).
// Implements the full key derivation chain: Early Secret -> Handshake
// Secret -> Master Secret, producing traffic keys at each phase.

use super::crypto::sha256::{Sha256, hkdf_expand, hkdf_extract};

// ── Traffic Keys ────────────────────────────────────────────────────

/// AES-128-GCM traffic key material derived from a traffic secret.
pub struct TrafficKeys {
    pub key: [u8; 16], // AES-128 key
    pub iv: [u8; 12],  // GCM nonce / IV
}

// ── HKDF-Expand-Label (RFC 8446 section 7.1) ───────────────────────

/// HKDF-Expand-Label as defined in RFC 8446 section 7.1.
///
/// ```text
/// HKDF-Expand-Label(Secret, Label, Context, Length) =
///     HKDF-Expand(Secret, HkdfLabel, Length)
///
/// HkdfLabel = length (2 bytes, big-endian)
///           || label_len (1 byte)
///           || "tls13 " || Label
///           || context_len (1 byte)
///           || Context
/// ```
///
/// Public because codec tests may need it.
pub fn hkdf_expand_label(secret: &[u8], label: &[u8], context: &[u8], out: &mut [u8]) -> crate::Result<()> {
    // Build HkdfLabel info structure
    let tls13_prefix = b"tls13 ";
    let full_label_len = tls13_prefix.len() + label.len();

    // info = length(2) || label_len(1) || "tls13 " || label || context_len(1) || context
    let info_len = 2 + 1 + full_label_len + 1 + context.len();
    let mut info = Vec::with_capacity(info_len);

    // Length field: output length as 2-byte big-endian
    info.push((out.len() >> 8) as u8);
    info.push(out.len() as u8);

    // Label: length prefix + "tls13 " + label
    info.push(full_label_len as u8);
    info.extend_from_slice(tls13_prefix);
    info.extend_from_slice(label);

    // Context: length prefix + context bytes
    info.push(context.len() as u8);
    info.extend_from_slice(context);

    hkdf_expand(secret, &info, out)
}

// ── Derive-Secret (RFC 8446 section 7.1) ────────────────────────────

/// Derive-Secret(Secret, Label, Messages) =
///     HKDF-Expand-Label(Secret, Label, Hash(Messages), Hash.length)
///
/// For our use the caller pre-computes the transcript hash, so `context`
/// is already Hash(Messages).
fn derive_secret(secret: &[u8], label: &[u8], transcript_hash: &[u8; 32]) -> crate::Result<[u8; 32]> {
    let mut out = [0u8; 32];
    hkdf_expand_label(secret, label, transcript_hash, &mut out)?;
    Ok(out)
}

/// Expand a traffic secret into AES-128-GCM key + IV.
fn traffic_keys(secret: &[u8; 32]) -> crate::Result<TrafficKeys> {
    let mut key = [0u8; 16];
    let mut iv = [0u8; 12];
    hkdf_expand_label(secret, b"key", &[], &mut key)?;
    hkdf_expand_label(secret, b"iv", &[], &mut iv)?;
    Ok(TrafficKeys { key, iv })
}

// ── Key Schedule ────────────────────────────────────────────────────

/// TLS 1.3 key schedule state machine.
///
/// Phases:
///   1. `new()` — Early Secret (no PSK, zeros)
///   2. `derive_handshake_secrets()` — ECDHE shared secret + hello hash
///   3. `derive_app_secrets()` — full handshake hash
impl Default for KeySchedule {
    fn default() -> Self {
        Self::new()
    }
}

pub struct KeySchedule {
    early_secret: [u8; 32],
    handshake_secret: [u8; 32],
    master_secret: [u8; 32],
    client_hs_secret: [u8; 32],
    server_hs_secret: [u8; 32],
}

impl KeySchedule {
    /// Phase 1: compute the Early Secret from zero PSK.
    ///
    /// ```text
    /// Early Secret = HKDF-Extract(salt=0x00*32, IKM=0x00*32)
    /// ```
    pub fn new() -> Self {
        let zeros = [0u8; 32];
        let early_secret = hkdf_extract(&zeros, &zeros);
        Self {
            early_secret,
            handshake_secret: [0u8; 32],
            master_secret: [0u8; 32],
            client_hs_secret: [0u8; 32],
            server_hs_secret: [0u8; 32],
        }
    }

    /// Read-only access to the early secret (for testing).
    pub fn early_secret(&self) -> &[u8; 32] {
        &self.early_secret
    }

    /// Phase 2: derive handshake secrets from the ECDHE shared secret
    /// and the transcript hash of ClientHello..ServerHello.
    ///
    /// ```text
    /// derived_1 = Derive-Secret(early_secret, "derived", "")
    /// Handshake Secret = HKDF-Extract(salt=derived_1, ikm=shared_secret)
    /// client_hs_secret = Derive-Secret(HS, "c hs traffic", hello_hash)
    /// server_hs_secret = Derive-Secret(HS, "s hs traffic", hello_hash)
    /// ```
    ///
    /// Returns (client_traffic_keys, server_traffic_keys).
    pub fn derive_handshake_secrets(
        &mut self,
        shared_secret: &[u8; 32],
        hello_hash: &[u8; 32],
    ) -> crate::Result<(TrafficKeys, TrafficKeys)> {
        // Derive-Secret(early_secret, "derived", "") — hash of empty string
        let empty_hash = Sha256::hash(b"");
        let derived_1 = derive_secret(&self.early_secret, b"derived", &empty_hash)?;

        // Handshake Secret
        self.handshake_secret = hkdf_extract(&derived_1, shared_secret);

        // Client/Server handshake traffic secrets
        self.client_hs_secret = derive_secret(&self.handshake_secret, b"c hs traffic", hello_hash)?;
        self.server_hs_secret = derive_secret(&self.handshake_secret, b"s hs traffic", hello_hash)?;

        Ok((
            traffic_keys(&self.client_hs_secret)?,
            traffic_keys(&self.server_hs_secret)?,
        ))
    }

    /// Phase 3: derive application traffic secrets from the full
    /// handshake transcript hash (ClientHello..server Finished).
    ///
    /// ```text
    /// derived_2 = Derive-Secret(handshake_secret, "derived", "")
    /// Master Secret = HKDF-Extract(salt=derived_2, ikm=0x00*32)
    /// client_app_secret = Derive-Secret(MS, "c ap traffic", handshake_hash)
    /// server_app_secret = Derive-Secret(MS, "s ap traffic", handshake_hash)
    /// ```
    ///
    /// Returns (client_traffic_keys, server_traffic_keys).
    pub fn derive_app_secrets(&mut self, handshake_hash: &[u8; 32]) -> crate::Result<(TrafficKeys, TrafficKeys)> {
        let empty_hash = Sha256::hash(b"");
        let derived_2 = derive_secret(&self.handshake_secret, b"derived", &empty_hash)?;

        let zeros = [0u8; 32];
        self.master_secret = hkdf_extract(&derived_2, &zeros);

        let client_app_secret = derive_secret(&self.master_secret, b"c ap traffic", handshake_hash)?;
        let server_app_secret = derive_secret(&self.master_secret, b"s ap traffic", handshake_hash)?;

        Ok((
            traffic_keys(&client_app_secret)?,
            traffic_keys(&server_app_secret)?,
        ))
    }

    /// Compute the server Finished key for Finished message verification.
    ///
    /// ```text
    /// finished_key = HKDF-Expand-Label(server_hs_secret, "finished", "", 32)
    /// ```
    pub fn server_finished_key(&self) -> crate::Result<[u8; 32]> {
        let mut key = [0u8; 32];
        hkdf_expand_label(&self.server_hs_secret, b"finished", &[], &mut key)?;
        Ok(key)
    }

    /// Compute the client Finished key for Finished message verification.
    ///
    /// ```text
    /// finished_key = HKDF-Expand-Label(client_hs_secret, "finished", "", 32)
    /// ```
    pub fn client_finished_key(&self) -> crate::Result<[u8; 32]> {
        let mut key = [0u8; 32];
        hkdf_expand_label(&self.client_hs_secret, b"finished", &[], &mut key)?;
        Ok(key)
    }
}
