//! Shared discovery path generation for commands and skills.

use std::path::{Path, PathBuf};

/// Build the list of discovery paths for a given subdirectory.
///
/// Priority order (highest first):
/// 1. `<cwd>/.krew/<subdir>/`   — project, krew-specific
/// 2. `<cwd>/.agents/<subdir>/` — project, cross-client
/// 3. `<cwd>/.claude/<subdir>/` — project, Claude Code compat
/// 4. `<home>/.krew/<subdir>/`  — user, krew-specific
/// 5. `<home>/.agents/<subdir>/` — user, cross-client
/// 6. `<home>/.claude/<subdir>/` — user, Claude Code compat
///
/// If the home directory cannot be determined, only project-level paths
/// (1-3) are returned.
pub fn discovery_paths(cwd: &Path, subdir: &str) -> Vec<PathBuf> {
    let mut paths = vec![
        cwd.join(".krew").join(subdir),
        cwd.join(".agents").join(subdir),
        cwd.join(".claude").join(subdir),
    ];
    if let Some(home) = dirs_home() {
        paths.push(home.join(".krew").join(subdir));
        paths.push(home.join(".agents").join(subdir));
        paths.push(home.join(".claude").join(subdir));
    }
    paths
}

/// Get the user's home directory.
fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}
