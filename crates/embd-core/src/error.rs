use thiserror::Error as ThisError;

/// Unified error type for all embd crates.
/// Each subsystem has its own variant so callers can match on category.
#[derive(Debug, ThisError)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Path error: {0}")]
    Path(String),

    #[error("Access denied: {0}")]
    AccessDenied(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Editor error: {0}")]
    Editor(String),

    #[error("Git error: {0}")]
    Git(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("{0}")]
    Other(String),
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Serialization(e.to_string())
    }
}

/// Convert to a String for Tauri command compatibility.
/// The existing Tauri commands return Result<T, String>.
impl From<Error> for String {
    fn from(e: Error) -> String {
        e.to_string()
    }
}
