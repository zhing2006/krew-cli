//! Project-level instructions file (AGENTS.md) discovery and loading.

use std::path::Path;

use crate::{PROJECT_INSTRUCTIONS_FILENAME, PROJECT_INSTRUCTIONS_MAX_SIZE};

/// Load project instructions by walking from `cwd` up to the filesystem root,
/// collecting all `AGENTS.md` files found along the way.
///
/// Files are merged ancestor-first (root → cwd) with a blank line separator.
/// Returns `None` if no instruction files are found.
/// Non-UTF-8 files are skipped with a warning log.
/// Files exceeding 100KB are truncated.
pub fn load_project_instructions(cwd: &Path) -> Result<Option<String>, std::io::Error> {
    let mut parts: Vec<String> = Vec::new();
    let mut dir = Some(cwd);

    // Collect paths from cwd upward so we can reverse for ancestor-first order.
    let mut search_dirs: Vec<&Path> = Vec::new();
    while let Some(d) = dir {
        search_dirs.push(d);
        dir = d.parent();
    }
    // Reverse: ancestor first, cwd last.
    search_dirs.reverse();

    for d in search_dirs {
        let file_path = d.join(PROJECT_INSTRUCTIONS_FILENAME);
        if !file_path.is_file() {
            continue;
        }

        let raw = match std::fs::read(&file_path) {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::warn!(
                    path = %file_path.display(),
                    error = %e,
                    "Failed to read project instructions file, skipping"
                );
                continue;
            }
        };

        let content = match String::from_utf8(raw) {
            Ok(s) => s,
            Err(_) => {
                tracing::warn!(
                    path = %file_path.display(),
                    "Project instructions file is not valid UTF-8, skipping"
                );
                continue;
            }
        };

        if content.len() > PROJECT_INSTRUCTIONS_MAX_SIZE {
            let truncated = truncate_utf8(&content, PROJECT_INSTRUCTIONS_MAX_SIZE);
            let mut result = truncated.to_string();
            result.push_str("\n\n[WARNING: File truncated at 100KB limit]");
            parts.push(result);
        } else {
            parts.push(content);
        }
    }

    if parts.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parts.join("\n\n")))
    }
}

/// Truncate a UTF-8 string to at most `max_bytes` bytes on a char boundary.
fn truncate_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    // Find the largest char boundary <= max_bytes.
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}
