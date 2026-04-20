// TLS 1.2 key schedule — RFC 5246 §5
//
// PRF, master secret, and key block derivation using HMAC-SHA256.

use crate::core::crypto::sha256::hmac_sha256;

/// P_hash(secret, seed) using HMAC-SHA256 — RFC 5246 §5.
/// Expands secret and seed into out.len() bytes.
pub fn p_hash(secret: &[u8], seed: &[u8], out: &mut [u8]) {
    let mut a = hmac_sha256(secret, seed); // A(1)
    let mut offset = 0;
    while offset < out.len() {
        let mut input = Vec::with_capacity(32 + seed.len());
        input.extend_from_slice(&a);
        input.extend_from_slice(seed);
        let block = hmac_sha256(secret, &input);
        let n = (out.len() - offset).min(32);
        out[offset..offset + n].copy_from_slice(&block[..n]);
        offset += n;
        if offset < out.len() {
            a = hmac_sha256(secret, &a); // A(i+1)
        }
    }
}

/// PRF(secret, label, seed) = P_SHA256(secret, label || seed) — RFC 5246 §5.
pub fn prf(secret: &[u8], label: &[u8], seed: &[u8], out: &mut [u8]) {
    let mut label_seed = Vec::with_capacity(label.len() + seed.len());
    label_seed.extend_from_slice(label);
    label_seed.extend_from_slice(seed);
    p_hash(secret, &label_seed, out);
}

/// master_secret = PRF(pre_master_secret, "master secret", ClientRandom || ServerRandom)
/// Returns 48 bytes (384 bits).
pub fn master_secret(
    pre_master: &[u8],
    client_random: &[u8; 32],
    server_random: &[u8; 32],
) -> [u8; 48] {
    let mut seed = [0u8; 64];
    seed[..32].copy_from_slice(client_random);
    seed[32..].copy_from_slice(server_random);
    let mut out = [0u8; 48];
    prf(pre_master, b"master secret", &seed, &mut out);
    out
}

/// KeyBlock holds the derived cipher keys and IVs.
/// For AES-128-CBC-SHA (40 bytes total):
/// - client_write_key: 16 bytes (AES-128)
/// - server_write_key: 16 bytes (AES-128)
/// - client_write_iv: 4 bytes
/// - server_write_iv: 4 bytes
pub struct KeyBlock {
    pub client_write_key: [u8; 16],
    pub server_write_key: [u8; 16],
    pub client_write_iv: [u8; 4],
    pub server_write_iv: [u8; 4],
}

/// key_block = PRF(master, "key expansion", ServerRandom || ClientRandom)
/// Derives encryption keys and IVs for client and server.
pub fn derive_key_block(
    master: &[u8; 48],
    server_random: &[u8; 32],
    client_random: &[u8; 32],
) -> KeyBlock {
    let mut seed = [0u8; 64];
    seed[..32].copy_from_slice(server_random); // server first for key expansion
    seed[32..].copy_from_slice(client_random);
    let mut kb = [0u8; 40]; // 16+16+4+4
    prf(master, b"key expansion", &seed, &mut kb);
    let mut client_write_key = [0u8; 16];
    let mut server_write_key = [0u8; 16];
    let mut client_write_iv = [0u8; 4];
    let mut server_write_iv = [0u8; 4];
    client_write_key.copy_from_slice(&kb[0..16]);
    server_write_key.copy_from_slice(&kb[16..32]);
    client_write_iv.copy_from_slice(&kb[32..36]);
    server_write_iv.copy_from_slice(&kb[36..40]);
    KeyBlock {
        client_write_key,
        server_write_key,
        client_write_iv,
        server_write_iv,
    }
}

/// Finished verify data = PRF(master, label, transcript_hash)[..12]
/// Returns 12 bytes of Finished message verify data.
pub fn finished_verify_data(
    master: &[u8; 48],
    label: &[u8],
    transcript_hash: &[u8; 32],
) -> [u8; 12] {
    let mut out = [0u8; 12];
    prf(master, label, transcript_hash, &mut out);
    out
}
