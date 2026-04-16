pub mod error;
pub mod json;
pub mod terminal;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;
