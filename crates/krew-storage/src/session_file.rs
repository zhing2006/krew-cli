use std::path::Path;

use crate::StorageError;

type Result<T> = std::result::Result<T, StorageError>;

/// Load a session from a TOML file.
pub fn load_session(_path: &Path) -> Result<String> {
    todo!()
}

/// Save a session to a TOML file.
pub fn save_session(_path: &Path, _data: &str) -> Result<()> {
    todo!()
}

/// List all session files in the given directory.
pub fn list_sessions(_dir: &Path) -> Result<Vec<String>> {
    todo!()
}
