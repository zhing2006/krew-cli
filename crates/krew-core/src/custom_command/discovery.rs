//! Scan multiple directories for `.md` command files and build a command registry.

use std::path::Path;

use super::CustomCommandRegistry;
use super::parser::parse_command_file;
use crate::discovery::discovery_paths;

/// Scan all discovery paths for custom commands and build a registry.
///
/// Priority order (first-found wins on name collisions):
/// `.krew > .agents > .claude`, project > user.
pub fn discover_commands(cwd: &Path) -> CustomCommandRegistry {
    let mut registry = CustomCommandRegistry::new();
    for dir in discovery_paths(cwd, "commands") {
        if dir.is_dir() {
            scan_dir(&dir, &dir, &mut registry);
        }
    }
    registry
}

/// Recursively scan a directory for `.md` files.
fn scan_dir(dir: &Path, base: &Path, registry: &mut CustomCommandRegistry) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!(path = %dir.display(), error = %e, "Failed to read commands directory");
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, base, registry);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            // Build command name from relative path.
            let rel = path.strip_prefix(base).unwrap_or(&path);
            let name = rel
                .with_extension("")
                .to_string_lossy()
                .replace(['/', '\\'], ":");
            // First-found wins: skip if already registered from a higher-priority path.
            if registry.lookup(&name).is_some() {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let cmd = parse_command_file(&name, &content);
                    registry.insert(cmd);
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "Failed to read command file");
                }
            }
        }
    }
}
