pub mod agent;
pub mod runtime;
pub mod llm;
pub mod error;
pub mod event;
pub mod json;
pub mod net;
pub mod repl;
pub mod terminal;
pub mod tui;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;
