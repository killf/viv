pub mod agent;
pub mod bus;
pub mod core;
pub mod memory;
pub mod llm;
pub mod error;
pub mod mcp;
pub mod tui;
pub mod permissions;
pub mod tools;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;
