use crate::core::json::JsonValue;
use std::future::Future;
use std::pin::Pin;

/// Transport layer for MCP communication.
///
/// Implementations handle the actual sending/receiving of JSON-RPC messages
/// over a specific transport mechanism (stdio, HTTP+SSE, etc.).
pub trait Transport: Send {
    fn send(
        &mut self,
        msg: JsonValue,
    ) -> Pin<Box<dyn Future<Output = crate::Result<()>> + Send + '_>>;
    fn recv(&mut self) -> Pin<Box<dyn Future<Output = crate::Result<JsonValue>> + Send + '_>>;
    fn close(&mut self) -> Pin<Box<dyn Future<Output = crate::Result<()>> + Send + '_>>;
}
