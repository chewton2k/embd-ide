pub mod error;
pub mod event;
pub mod types;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;
