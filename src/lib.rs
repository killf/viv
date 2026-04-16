pub mod api;
pub mod error;
pub mod event;
pub mod json;
pub mod net;
pub mod repl;
pub mod terminal;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;
