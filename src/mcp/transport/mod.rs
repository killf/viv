pub mod stdio;
pub mod sse;
pub mod http;
pub mod ws;

use crate::core::json::JsonValue;
use std::future::Future;
use std::pin::Pin;

/// Transport layer for MCP communication.
///
/// Uses `Pin<Box<dyn Future>>` because Transport will be used as a trait object
/// inside `McpClientKind` enum.
pub trait Transport: Send {
    fn send(&mut self, msg: JsonValue) -> Pin<Box<dyn Future<Output = crate::Result<()>> + Send + '_>>;
    fn recv(&mut self) -> Pin<Box<dyn Future<Output = crate::Result<JsonValue>> + Send + '_>>;
    fn close(&mut self) -> Pin<Box<dyn Future<Output = crate::Result<()>> + Send + '_>>;
}
