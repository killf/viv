pub mod agent;
pub mod config;
pub mod core;
pub mod error;
pub mod llm;
pub mod log;
pub mod lsp;
pub mod mcp;
pub mod memory;
pub mod permissions;
pub mod skill;
pub mod tools;
pub mod tui;

pub use error::Error;
pub use tui::qrcode;
pub type Result<T> = std::result::Result<T, Error>;
