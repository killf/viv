// TLS 1.3 codec — serialization/deserialization of TLS structures
//
// Handles encoding ClientHello, Finished, ChangeCipherSpec, and
// decoding ServerHello, EncryptedExtensions, Certificate,
// CertificateVerify, Finished from raw handshake bytes.

// ── Content types ──────────────────────────────────────────────────
pub const CHANGE_CIPHER_SPEC: u8 = 0x14;
pub const ALERT: u8 = 0x15;
pub const HANDSHAKE: u8 = 0x16;
pub const APPLICATION_DATA: u8 = 0x17;

// ── Handshake message types ────────────────────────────────────────
pub const CLIENT_HELLO: u8 = 0x01;
pub const SERVER_HELLO: u8 = 0x02;
pub const ENCRYPTED_EXTENSIONS: u8 = 0x08;
pub const CERTIFICATE: u8 = 0x0b;
pub const CERTIFICATE_VERIFY: u8 = 0x0f;
pub const FINISHED: u8 = 0x14;

// ── Extension types ────────────────────────────────────────────────
pub const EXT_SERVER_NAME: u16 = 0;
pub const EXT_SUPPORTED_GROUPS: u16 = 10;
pub const EXT_SIGNATURE_ALGORITHMS: u16 = 13;
pub const EXT_SUPPORTED_VERSIONS: u16 = 43;
pub const EXT_KEY_SHARE: u16 = 51;

// ── Decoded messages ───────────────────────────────────────────────

pub enum HandshakeMessage {
    ServerHello(ServerHello),
    EncryptedExtensions,
    Certificate(Vec<Vec<u8>>),
    CertificateVerify { scheme: u16, signature: Vec<u8> },
    Finished { verify_data: [u8; 32] },
}

pub struct ServerHello {
    pub version: u16,                    // 0x0303 TLS1.2 or 0x0304 TLS1.3
    pub random: [u8; 32],
    pub cipher_suite: u16,
    pub x25519_public: Option<[u8; 32]>, // TLS 1.3 only
}

// ── Helpers ────────────────────────────────────────────────────────

fn push_u16(out: &mut Vec<u8>, v: u16) {
    out.push((v >> 8) as u8);
    out.push(v as u8);
}

fn push_u24(out: &mut Vec<u8>, v: u32) {
    out.push((v >> 16) as u8);
    out.push((v >> 8) as u8);
    out.push(v as u8);
}

fn read_u16(data: &[u8], pos: usize) -> crate::Result<u16> {
    if pos + 2 > data.len() {
        return Err(crate::Error::Tls("truncated u16".into()));
    }
    Ok(((data[pos] as u16) << 8) | (data[pos + 1] as u16))
}

fn read_u24(data: &[u8], pos: usize) -> crate::Result<u32> {
    if pos + 3 > data.len() {
        return Err(crate::Error::Tls("truncated u24".into()));
    }
    Ok(((data[pos] as u32) << 16) | ((data[pos + 1] as u32) << 8) | (data[pos + 2] as u32))
}

// ── Encode: ClientHello ────────────────────────────────────────────

/// Encode a TLS 1.3 ClientHello handshake message (without the TLS
/// record header). The caller wraps this in a record via RecordLayer.
pub fn encode_client_hello(
    random: &[u8; 32],
    session_id: &[u8; 32],
    server_name: &str,
    x25519_pub: &[u8; 32],
    out: &mut Vec<u8>,
) {
    // Build extensions first so we know the total length
    let mut exts = Vec::new();
    encode_ext_server_name(server_name, &mut exts);
    encode_ext_supported_versions(&mut exts);
    encode_ext_supported_groups(&mut exts);
    encode_ext_key_share(x25519_pub, &mut exts);
    encode_ext_signature_algorithms(&mut exts);

    // ClientHello body (without handshake header)
    let mut body = Vec::new();

    // legacy_version: TLS 1.2
    push_u16(&mut body, 0x0303);

    // random (32 bytes)
    body.extend_from_slice(random);

    // session_id: length(1) + data(32)
    body.push(32);
    body.extend_from_slice(session_id);

    // cipher_suites: length(2) + 3 suites * 2 bytes each = 6 bytes
    push_u16(&mut body, 6);
    push_u16(&mut body, 0x1301); // TLS_AES_128_GCM_SHA256 (TLS 1.3)
    push_u16(&mut body, 0xC02B); // TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256
    push_u16(&mut body, 0xC02C); // TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256

    // compression_methods: length(1) + null
    body.push(1);
    body.push(0x00);

    // extensions: length(2) + data
    push_u16(&mut body, exts.len() as u16);
    body.extend_from_slice(&exts);

    // Handshake header: type(1) + length(3) + body
    out.push(CLIENT_HELLO);
    push_u24(out, body.len() as u32);
    out.extend_from_slice(&body);
}

// ── Encode: Finished ───────────────────────────────────────────────

/// Encode a TLS 1.3 Finished handshake message.
pub fn encode_finished(verify_data: &[u8; 32], out: &mut Vec<u8>) {
    out.push(FINISHED);
    push_u24(out, 32);
    out.extend_from_slice(verify_data);
}

// ── Encode: ChangeCipherSpec ───────────────────────────────────────

/// Encode a legacy ChangeCipherSpec message (1-byte payload: 0x01).
/// This is sent as a full TLS record (type 0x14), not a handshake message.
pub fn encode_change_cipher_spec(out: &mut Vec<u8>) {
    out.push(CHANGE_CIPHER_SPEC); // content type
    push_u16(out, 0x0303); // legacy version TLS 1.2
    push_u16(out, 1); // length = 1
    out.push(0x01); // CCS payload
}

// ── Extension encoders ─────────────────────────────────────────────

fn encode_ext_server_name(name: &str, out: &mut Vec<u8>) {
    let name_bytes = name.as_bytes();
    // Extension header
    push_u16(out, EXT_SERVER_NAME);
    // Extension data length = list_len(2) + entry_type(1) + name_len(2) + name
    let data_len = 2 + 1 + 2 + name_bytes.len();
    push_u16(out, data_len as u16);
    // Server Name List length
    let list_len = 1 + 2 + name_bytes.len();
    push_u16(out, list_len as u16);
    // Server name type: host_name (0)
    out.push(0);
    // Host name length + data
    push_u16(out, name_bytes.len() as u16);
    out.extend_from_slice(name_bytes);
}

fn encode_ext_supported_versions(out: &mut Vec<u8>) {
    push_u16(out, EXT_SUPPORTED_VERSIONS);
    push_u16(out, 5); // extension data length: list_len(1) + 2 versions * 2 bytes
    out.push(4);      // list length
    push_u16(out, 0x0304); // TLS 1.3
    push_u16(out, 0x0303); // TLS 1.2
}

fn encode_ext_supported_groups(out: &mut Vec<u8>) {
    push_u16(out, EXT_SUPPORTED_GROUPS);
    push_u16(out, 6); // ext data length
    push_u16(out, 4); // list length
    push_u16(out, 0x001d); // x25519
    push_u16(out, 0x0017); // secp256r1 (P-256) for TLS 1.2
}

fn encode_ext_key_share(pub_key: &[u8; 32], out: &mut Vec<u8>) {
    push_u16(out, EXT_KEY_SHARE);
    // extension data = shares_len(2) + group(2) + key_len(2) + key(32) = 38
    push_u16(out, 38);
    // client_shares length = group(2) + key_len(2) + key(32) = 36
    push_u16(out, 36);
    push_u16(out, 0x001d); // x25519
    push_u16(out, 32); // key exchange length
    out.extend_from_slice(pub_key);
}

fn encode_ext_signature_algorithms(out: &mut Vec<u8>) {
    let schemes: &[u16] = &[
        0x0403, // ecdsa_secp256r1_sha256
        0x0804, // rsa_pss_rsae_sha256
        0x0401, // rsa_pkcs1_sha256
        0x0503, // ecdsa_secp384r1_sha384
        0x0805, // rsa_pss_rsae_sha384
        0x0501, // rsa_pkcs1_sha384
        0x0806, // rsa_pss_rsae_sha512
        0x0601, // rsa_pkcs1_sha512
    ];
    push_u16(out, EXT_SIGNATURE_ALGORITHMS);
    let list_len = schemes.len() * 2;
    push_u16(out, (list_len + 2) as u16); // ext data = list_len(2) + schemes
    push_u16(out, list_len as u16);
    for &s in schemes {
        push_u16(out, s);
    }
}

// ── Decode: handshake message ──────────────────────────────────────

/// Extract the negotiated version from a raw ServerHello handshake message.
/// `msg_bytes` = type(1) + len(3) + body.
/// Returns 0x0303 for TLS 1.2, 0x0304 for TLS 1.3.
pub fn peek_server_hello_version(msg_bytes: &[u8]) -> crate::Result<u16> {
    if msg_bytes.len() < 4 {
        return Err(crate::Error::Tls("ServerHello too short to peek version".into()));
    }
    let body_len = read_u24(msg_bytes, 1)? as usize;
    if msg_bytes.len() < 4 + body_len {
        return Err(crate::Error::Tls("ServerHello body truncated".into()));
    }
    let body = &msg_bytes[4..4 + body_len];
    match decode_server_hello(body)? {
        HandshakeMessage::ServerHello(sh) => Ok(sh.version),
        _ => Err(crate::Error::Tls("expected ServerHello".into())),
    }
}

/// Decode a handshake message from raw bytes (starting at the handshake
/// header: type(1) + length(3) + body). Returns the parsed message.
pub fn decode_handshake(data: &[u8]) -> crate::Result<HandshakeMessage> {
    if data.len() < 4 {
        return Err(crate::Error::Tls("handshake too short".into()));
    }
    let msg_type = data[0];
    let length = read_u24(data, 1)? as usize;

    if data.len() < 4 + length {
        return Err(crate::Error::Tls("handshake body truncated".into()));
    }
    let body = &data[4..4 + length];

    match msg_type {
        SERVER_HELLO => decode_server_hello(body),
        ENCRYPTED_EXTENSIONS => Ok(HandshakeMessage::EncryptedExtensions),
        CERTIFICATE => decode_certificate(body),
        CERTIFICATE_VERIFY => decode_certificate_verify(body),
        FINISHED => decode_finished(body),
        _ => Err(crate::Error::Tls(format!(
            "unknown handshake type: 0x{:02x}",
            msg_type
        ))),
    }
}

// ── Decode: ServerHello ────────────────────────────────────────────

fn decode_server_hello(body: &[u8]) -> crate::Result<HandshakeMessage> {
    if body.len() < 35 {
        return Err(crate::Error::Tls("ServerHello too short".into()));
    }
    let mut pos = 0;
    let legacy_version = read_u16(body, pos)?;
    pos += 2;
    let mut random = [0u8; 32];
    random.copy_from_slice(&body[pos..pos + 32]);
    pos += 32;
    let sid_len = body[pos] as usize;
    pos += 1 + sid_len;
    if pos + 3 > body.len() {
        return Err(crate::Error::Tls("ServerHello truncated at cipher".into()));
    }
    let cipher_suite = read_u16(body, pos)?;
    pos += 2;
    pos += 1; // compression

    let mut x25519_public: Option<[u8; 32]> = None;
    let mut negotiated_version = legacy_version;

    if pos + 2 <= body.len() {
        let exts_len = read_u16(body, pos)? as usize;
        pos += 2;
        let exts_end = pos + exts_len;
        if exts_end > body.len() {
            return Err(crate::Error::Tls("ServerHello extensions truncated".into()));
        }
        while pos + 4 <= exts_end {
            let ext_type = read_u16(body, pos)?;
            let ext_len = read_u16(body, pos + 2)? as usize;
            pos += 4;
            if pos + ext_len > exts_end {
                return Err(crate::Error::Tls("extension data truncated".into()));
            }
            match ext_type {
                EXT_KEY_SHARE if ext_len >= 4 => {
                    let key_len = read_u16(body, pos + 2)? as usize;
                    if key_len == 32 && pos + 4 + 32 <= exts_end {
                        let mut k = [0u8; 32];
                        k.copy_from_slice(&body[pos + 4..pos + 4 + 32]);
                        x25519_public = Some(k);
                    }
                }
                EXT_SUPPORTED_VERSIONS if ext_len >= 2 => {
                    negotiated_version = read_u16(body, pos)?;
                    if negotiated_version != 0x0304 && negotiated_version != 0x0303 {
                        return Err(crate::Error::Tls(format!(
                            "unsupported TLS version: 0x{:04x}", negotiated_version
                        )));
                    }
                }
                _ => {}
            }
            pos += ext_len;
        }
    }

    if negotiated_version == 0x0304 && x25519_public.is_none() {
        return Err(crate::Error::Tls("TLS 1.3 ServerHello missing key_share".into()));
    }

    Ok(HandshakeMessage::ServerHello(ServerHello {
        version: negotiated_version,
        random,
        cipher_suite,
        x25519_public,
    }))
}

// ── Decode: Certificate ────────────────────────────────────────────

fn decode_certificate(body: &[u8]) -> crate::Result<HandshakeMessage> {
    if body.is_empty() {
        return Err(crate::Error::Tls("Certificate message empty".into()));
    }

    let mut pos: usize = 0;

    // request_context length (1 byte) + request_context
    let ctx_len = body[pos] as usize;
    pos += 1 + ctx_len;

    // certificate_list length (3 bytes)
    if pos + 3 > body.len() {
        return Err(crate::Error::Tls("Certificate list truncated".into()));
    }
    let list_len = read_u24(body, pos)? as usize;
    pos += 3;

    let list_end = pos + list_len;
    if list_end > body.len() {
        return Err(crate::Error::Tls("Certificate list data truncated".into()));
    }

    let mut certs = Vec::new();
    while pos + 3 <= list_end {
        let cert_len = read_u24(body, pos)? as usize;
        pos += 3;
        if pos + cert_len > list_end {
            return Err(crate::Error::Tls("Certificate entry truncated".into()));
        }
        certs.push(body[pos..pos + cert_len].to_vec());
        pos += cert_len;

        // extensions per certificate entry (2-byte length + data)
        if pos + 2 <= list_end {
            let ext_len = read_u16(body, pos)? as usize;
            pos += 2 + ext_len;
        }
    }

    Ok(HandshakeMessage::Certificate(certs))
}

// ── Decode: CertificateVerify ──────────────────────────────────────

fn decode_certificate_verify(body: &[u8]) -> crate::Result<HandshakeMessage> {
    if body.len() < 4 {
        return Err(crate::Error::Tls("CertificateVerify too short".into()));
    }
    let scheme = read_u16(body, 0)?;
    let sig_len = read_u16(body, 2)? as usize;
    if body.len() < 4 + sig_len {
        return Err(crate::Error::Tls("CertificateVerify sig truncated".into()));
    }
    let signature = body[4..4 + sig_len].to_vec();

    Ok(HandshakeMessage::CertificateVerify { scheme, signature })
}

// ── Decode: Finished ───────────────────────────────────────────────

fn decode_finished(body: &[u8]) -> crate::Result<HandshakeMessage> {
    if body.len() < 32 {
        return Err(crate::Error::Tls("Finished verify_data too short".into()));
    }
    let mut verify_data = [0u8; 32];
    verify_data.copy_from_slice(&body[..32]);
    Ok(HandshakeMessage::Finished { verify_data })
}
