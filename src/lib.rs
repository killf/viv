pub mod agent;
pub mod bus;
pub mod core;
pub mod error;
pub mod llm;
pub mod lsp;
pub mod mcp;
pub mod memory;
pub mod permissions;
pub mod tools;
pub mod tui;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;
