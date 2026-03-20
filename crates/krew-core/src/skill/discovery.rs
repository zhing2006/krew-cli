//! Skill discovery: scan directories for SKILL.md files and parse them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tracing::warn;

use super::types::{SkillError, SkillRecord};

/// YAML frontmatter structure for SKILL.md.
#[derive(Deserialize)]
struct SkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
    #[serde(default)]
    compatibility: Option<String>,
    #[serde(default)]
    metadata: Option<HashMap<String, String>>,
}

/// Directories to skip during skill scanning.
const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "__pycache__",
    ".venv",
    "dist",
    "build",
];

/// Maximum directory depth for skill scanning.
const MAX_SCAN_DEPTH: usize = 4;

/// Parse a single SKILL.md file into a `SkillRecord`.
pub fn parse_skill_md(path: &Path) -> Result<SkillRecord, SkillError> {
    let content = std::fs::read_to_string(path)?;

    // Extract YAML frontmatter between --- delimiters.
    let (yaml_str, _body) = extract_frontmatter(&content)
        .ok_or_else(|| SkillError::InvalidFrontmatter("no YAML frontmatter found".into()))?;

    let frontmatter: SkillFrontmatter = serde_yaml::from_str(yaml_str).map_err(|e| {
        // Attempt lenient parsing for unquoted colons.
        SkillError::InvalidFrontmatter(e.to_string())
    })?;

    let name = frontmatter
        .name
        .filter(|n| !n.is_empty())
        .ok_or_else(|| SkillError::MissingField("name".into()))?;

    let description = frontmatter
        .description
        .filter(|d| !d.is_empty())
        .ok_or_else(|| SkillError::MissingField("description".into()))?;

    let base_dir = path.parent().unwrap_or(path).to_path_buf();

    // Warn if name doesn't match directory name (lenient: still load).
    if let Some(dir_name) = base_dir
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|d| *d != name)
    {
        warn!(
            skill_name = %name,
            dir_name = %dir_name,
            "skill name does not match directory name"
        );
    }

    // Warn if name exceeds 64 characters (lenient: still load).
    if name.len() > 64 {
        warn!(
            skill_name = %name,
            "skill name exceeds 64 characters"
        );
    }

    Ok(SkillRecord {
        name,
        description,
        location: path.to_path_buf(),
        base_dir,
        compatibility: frontmatter.compatibility,
        metadata: frontmatter.metadata,
    })
}

/// Extract YAML frontmatter and body from a SKILL.md file content.
///
/// Returns `(yaml_str, body_str)` if frontmatter is found.
pub fn extract_frontmatter(content: &str) -> Option<(&str, &str)> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return None;
    }

    // Find the closing --- after the opening one.
    let after_open = &trimmed[3..];
    let close_pos = after_open.find("\n---")?;
    let yaml = &after_open[..close_pos].trim_start_matches('\n');

    // Body starts after the closing --- line.
    let rest = &after_open[close_pos + 4..];
    let body = rest.trim_start_matches(['\r', '\n']);

    Some((yaml, body))
}

/// Discover all skills from default and extra paths.
///
/// Default scan paths (in priority order):
/// 1. `<cwd>/.krew/skills/` (project, krew-specific)
/// 2. `<cwd>/.agents/skills/` (project, cross-client)
/// 3. `<cwd>/.claude/skills/` (project, Claude Code compat)
/// 4. `<home>/.krew/skills/` (user, krew-specific)
/// 5. `<home>/.agents/skills/` (user, cross-client)
/// 6. `<home>/.claude/skills/` (user, Claude Code compat)
///
/// First-found wins on name collisions.
pub fn discover_skills(cwd: &Path, extra_paths: &[PathBuf]) -> Vec<SkillRecord> {
    let mut seen: HashMap<String, SkillRecord> = HashMap::new();

    let mut scan_paths = crate::discovery::discovery_paths(cwd, "skills");

    for extra in extra_paths {
        scan_paths.push(extra.clone());
    }

    for scan_dir in &scan_paths {
        if !scan_dir.is_dir() {
            continue;
        }
        scan_directory(scan_dir, &mut seen);
    }

    seen.into_values().collect()
}

/// Scan a single directory for skill subdirectories containing SKILL.md.
fn scan_directory(dir: &Path, seen: &mut HashMap<String, SkillRecord>) {
    let walker = walkdir::WalkDir::new(dir)
        .max_depth(MAX_SCAN_DEPTH)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            // Skip hidden dirs and known non-skill dirs.
            if entry.file_type().is_dir() {
                let name = entry.file_name().to_string_lossy();
                !SKIP_DIRS.contains(&name.as_ref())
            } else {
                true
            }
        });

    for entry in walker.flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.file_name() != "SKILL.md" {
            continue;
        }

        let path = entry.path();
        match parse_skill_md(path) {
            Ok(record) => {
                if seen.contains_key(&record.name) {
                    warn!(
                        skill_name = %record.name,
                        shadowed_by = %seen[&record.name].location.display(),
                        new_location = %record.location.display(),
                        "skill name collision, keeping higher-priority version"
                    );
                } else {
                    seen.insert(record.name.clone(), record);
                }
            }
            Err(e) => {
                warn!(
                    path = %path.display(),
                    error = %e,
                    "failed to parse SKILL.md, skipping"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_extract_frontmatter_basic() {
        let content =
            "---\nname: test\ndescription: A test skill\n---\n\n# Instructions\nDo something.";
        let (yaml, body) = extract_frontmatter(content).unwrap();
        assert!(yaml.contains("name: test"));
        assert!(body.contains("# Instructions"));
    }

    #[test]
    fn test_extract_frontmatter_missing() {
        let content = "# No frontmatter here";
        assert!(extract_frontmatter(content).is_none());
    }

    #[test]
    fn test_parse_skill_md_valid() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("my-skill");
        fs::create_dir(&skill_dir).unwrap();
        let skill_md = skill_dir.join("SKILL.md");
        fs::write(
            &skill_md,
            "---\nname: my-skill\ndescription: A test skill for testing.\n---\n\n# How to use\nFollow these steps.",
        )
        .unwrap();

        let record = parse_skill_md(&skill_md).unwrap();
        assert_eq!(record.name, "my-skill");
        assert_eq!(record.description, "A test skill for testing.");
        assert_eq!(record.base_dir, skill_dir);
    }

    #[test]
    fn test_parse_skill_md_missing_description() {
        let dir = tempfile::tempdir().unwrap();
        let skill_md = dir.path().join("SKILL.md");
        fs::write(&skill_md, "---\nname: broken\n---\n\nBody text.").unwrap();

        let err = parse_skill_md(&skill_md).unwrap_err();
        assert!(err.to_string().contains("description"));
    }

    #[test]
    fn test_parse_skill_md_with_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("advanced");
        fs::create_dir(&skill_dir).unwrap();
        let skill_md = skill_dir.join("SKILL.md");
        fs::write(
            &skill_md,
            "---\nname: advanced\ndescription: An advanced skill.\ncompatibility: Requires git\nmetadata:\n  author: test-org\n  version: \"1.0\"\n---\n\nInstructions here.",
        )
        .unwrap();

        let record = parse_skill_md(&skill_md).unwrap();
        assert_eq!(record.name, "advanced");
        assert_eq!(record.compatibility.as_deref(), Some("Requires git"));
        let meta = record.metadata.unwrap();
        assert_eq!(meta.get("author").unwrap(), "test-org");
    }

    #[test]
    fn test_discover_skills_from_directory() {
        let dir = tempfile::tempdir().unwrap();

        // Create .krew/skills/review/SKILL.md
        let review_dir = dir.path().join(".krew").join("skills").join("review");
        fs::create_dir_all(&review_dir).unwrap();
        fs::write(
            review_dir.join("SKILL.md"),
            "---\nname: review\ndescription: Code review skill.\n---\n\nReview code.",
        )
        .unwrap();

        // Create .agents/skills/search/SKILL.md
        let search_dir = dir.path().join(".agents").join("skills").join("search");
        fs::create_dir_all(&search_dir).unwrap();
        fs::write(
            search_dir.join("SKILL.md"),
            "---\nname: search\ndescription: Web search skill.\n---\n\nSearch the web.",
        )
        .unwrap();

        let skills = discover_skills(dir.path(), &[]);
        assert_eq!(skills.len(), 2);

        let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"review"));
        assert!(names.contains(&"search"));
    }

    #[test]
    fn test_discover_skills_priority_override() {
        let dir = tempfile::tempdir().unwrap();

        // Create higher-priority version in .krew/skills/
        let krew_dir = dir.path().join(".krew").join("skills").join("dupe");
        fs::create_dir_all(&krew_dir).unwrap();
        fs::write(
            krew_dir.join("SKILL.md"),
            "---\nname: dupe\ndescription: From krew dir.\n---\n\nKrew version.",
        )
        .unwrap();

        // Create lower-priority version in .agents/skills/
        let agents_dir = dir.path().join(".agents").join("skills").join("dupe");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::write(
            agents_dir.join("SKILL.md"),
            "---\nname: dupe\ndescription: From agents dir.\n---\n\nAgents version.",
        )
        .unwrap();

        let skills = discover_skills(dir.path(), &[]);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].description, "From krew dir.");
    }
}
