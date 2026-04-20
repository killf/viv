pub mod agent;
pub mod config;
pub mod core;
pub mod error;
pub mod llm;
pub mod lsp;
pub mod mcp;
pub mod memory;
pub mod permissions;
pub mod skill;
pub mod tools;
pub mod tui;

// Re-export log at the crate root so `$crate::log::` in macros and
// existing `viv::log::` paths in tests continue to work.
pub use core::log;

pub use error::Error;
pub use tui::qrcode;
pub type Result<T> = std::result::Result<T, Error>;
