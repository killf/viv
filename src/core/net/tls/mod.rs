// TLS 1.3 pure-Rust implementation (in progress)
//
// Legacy OpenSSL FFI is in `super::tls_legacy`. We re-export `TlsStream`
// so that existing callers (`llm.rs`, tests) continue to compile unchanged.

pub mod crypto;
pub mod key_schedule;
pub mod codec;
pub mod record;
pub mod handshake;
pub mod x509;

// Temporary re-export from legacy OpenSSL implementation
pub use super::tls_legacy::TlsStream;
