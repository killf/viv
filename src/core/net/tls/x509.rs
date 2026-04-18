// X.509 certificate verification stub
//
// TLS 1.3 requires validating the server's certificate chain and the
// CertificateVerify signature. For now we accept all certificates.
// A proper implementation would parse DER/ASN.1, verify the chain
// against system root CAs, check validity dates, and verify the
// CertificateVerify signature using the leaf certificate's public key.

/// Verify a certificate chain. Currently a no-op stub.
///
/// `_certs`: DER-encoded certificate chain (leaf first).
/// `_server_name`: expected hostname for SNI validation.
///
/// Returns Ok(()) unconditionally. A real implementation would:
/// 1. Parse each DER certificate (ASN.1)
/// 2. Verify the chain up to a trusted root CA
/// 3. Check notBefore / notAfter validity
/// 4. Verify the leaf certificate's subjectAltName matches server_name
pub fn verify_chain(_certs: &[Vec<u8>], _server_name: &str) -> crate::Result<()> {
    // TODO: implement real certificate validation
    Ok(())
}

/// Verify a CertificateVerify signature. Currently a no-op stub.
///
/// `_scheme`: signature algorithm (e.g., 0x0804 = rsa_pss_rsae_sha256)
/// `_signature`: the raw signature bytes
/// `_cert_der`: the leaf certificate in DER format
/// `_transcript_hash`: SHA-256 hash of the transcript up to Certificate
pub fn verify_signature(
    _scheme: u16,
    _signature: &[u8],
    _cert_der: &[u8],
    _transcript_hash: &[u8; 32],
) -> crate::Result<()> {
    // TODO: implement real signature verification
    Ok(())
}
