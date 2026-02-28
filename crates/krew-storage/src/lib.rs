pub mod session_file;

/// Errors that can occur during session storage operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("storage error: {0}")]
    Other(String),
}
