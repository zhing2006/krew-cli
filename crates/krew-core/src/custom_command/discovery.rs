//! Scan `.krew/commands/` directory and build a command registry.

use std::path::Path;

use super::CustomCommandRegistry;
use super::parser::parse_command_file;

/// Scan the `.krew/commands/` directory and build a registry of custom commands.
///
/// Files are mapped to command names by stripping the base path and `.md` extension,
/// then replacing path separators with `:`.
pub fn discover_commands(base_dir: &Path) -> CustomCommandRegistry {
    let commands_dir = base_dir.join(".krew").join("commands");
    let mut registry = CustomCommandRegistry::new();

    if !commands_dir.is_dir() {
        return registry;
    }

    scan_dir(&commands_dir, &commands_dir, &mut registry);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_flat_commands() {
        let tmp = tempfile::tempdir().unwrap();
        let cmd_dir = tmp.path().join(".krew").join("commands");
        fs::create_dir_all(&cmd_dir).unwrap();
        fs::write(
            cmd_dir.join("commit.md"),
            "---\ndescription: Make a commit\n---\n@coder commit\n",
        )
        .unwrap();
        fs::write(cmd_dir.join("review.md"), "@reviewer review this\n").unwrap();

        let registry = discover_commands(tmp.path());
        assert_eq!(registry.list().len(), 2);
        assert!(registry.lookup("commit").is_some());
        assert!(registry.lookup("review").is_some());
        assert_eq!(
            registry.lookup("commit").unwrap().description,
            "Make a commit"
        );
    }

    #[test]
    fn test_nested_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let cmd_dir = tmp.path().join(".krew").join("commands");
        let sub_dir = cmd_dir.join("git");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(sub_dir.join("push.md"), "@coder push\n").unwrap();

        let registry = discover_commands(tmp.path());
        assert!(registry.lookup("git:push").is_some());
    }

    #[test]
    fn test_non_md_files_ignored() {
        let tmp = tempfile::tempdir().unwrap();
        let cmd_dir = tmp.path().join(".krew").join("commands");
        fs::create_dir_all(&cmd_dir).unwrap();
        fs::write(cmd_dir.join("notes.txt"), "not a command").unwrap();
        fs::write(cmd_dir.join("real.md"), "a command\n").unwrap();

        let registry = discover_commands(tmp.path());
        assert_eq!(registry.list().len(), 1);
        assert!(registry.lookup("real").is_some());
    }

    #[test]
    fn test_missing_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = discover_commands(tmp.path());
        assert!(registry.is_empty());
    }
}
