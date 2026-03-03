pub mod history_file;
pub mod session_file;

/// Errors that can occur during session storage operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("storage error: {0}")]
    Other(String),
}
