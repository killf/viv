use std::fmt;

#[derive(Debug)]
pub enum Error {
    /// JSON parse errors
    Json(String),
    /// Terminal operation errors
    Terminal(String),
    /// Network/IO errors
    Io(std::io::Error),
    /// TLS/SSL errors
    Tls(String),
    /// HTTP protocol errors
    Http(String),
    /// LLM API errors (status code + message)
    LLM { status: u16, message: String },
    /// Tool execution errors
    Tool(String),
    /// JSON-RPC protocol errors
    JsonRpc { code: i64, message: String },
    /// MCP runtime errors
    Mcp { server: String, message: String },
    /// LSP runtime errors
    Lsp { server: String, message: String },
    /// QR code generation errors
    Qr(String),
    /// ASN.1/DER parse errors
    Asn1(String),
    /// Mutex poisoned (another thread panicked while holding the lock)
    LockPoisoned(String),
    /// Invariant violation (unexpected state that was previously a panic)
    Invariant(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Json(msg) => write!(f, "JSON error: {}", msg),
            Error::Terminal(msg) => write!(f, "terminal error: {}", msg),
            Error::Io(err) => write!(f, "IO error: {}", err),
            Error::Tls(msg) => write!(f, "TLS error: {}", msg),
            Error::Http(msg) => write!(f, "HTTP error: {}", msg),
            Error::LLM { status, message } => write!(f, "LLM error {}: {}", status, message),
            Error::Tool(msg) => write!(f, "tool error: {}", msg),
            Error::JsonRpc { code, message } => write!(f, "JSON-RPC error {}: {}", code, message),
            Error::Mcp { server, message } => write!(f, "MCP error [{}]: {}", server, message),
            Error::Lsp { server, message } => write!(f, "LSP error [{}]: {}", server, message),
            Error::Qr(msg) => write!(f, "QR code error: {}", msg),
            Error::Asn1(msg) => write!(f, "ASN.1 error: {}", msg),
            Error::LockPoisoned(msg) => write!(f, "lock poisoned: {}", msg),
            Error::Invariant(msg) => write!(f, "invariant violation: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}
