//! Sub-Agent discovery: scan directories for agent definition `.md` files.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use tracing::warn;

use super::types::{DEFAULT_SUB_AGENT_MAX_TURNS, SubAgentDef};
use crate::skill::extract_frontmatter;

/// YAML frontmatter for Sub-Agent definition files.
///
/// Claude Code compatible fields (tools, model, etc.) are captured by
/// `#[serde(flatten)]` into `_extra` and silently ignored.
#[derive(Deserialize)]
struct AgentFrontmatter {
    name: Option<String>,
    description: Option<String>,
    color: Option<String>,
    #[serde(alias = "maxTurns")]
    max_turns: Option<u32>,
    /// Catch-all for Claude Code fields we don't use.
    #[serde(flatten)]
    _extra: HashMap<String, serde_yaml::Value>,
}

/// Discover Sub-Agent definitions from standard directories.
///
/// Scans the following paths (in priority order):
/// 1. `<cwd>/.krew/agents/`
/// 2. `<cwd>/.agents/agents/`
/// 3. `<cwd>/.claude/agents/`
/// 4. `<home>/.krew/agents/`
/// 5. `<home>/.agents/agents/`
/// 6. `<home>/.claude/agents/`
///
/// Only top-level `*.md` files are scanned (non-recursive).
/// First-found-wins deduplication by name.
pub fn discover_sub_agents(cwd: &Path) -> Vec<SubAgentDef> {
    let scan_paths = crate::discovery::discovery_paths(cwd, "agents");
    let mut seen: HashMap<String, SubAgentDef> = HashMap::new();

    for dir in &scan_paths {
        if !dir.is_dir() {
            continue;
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }

            match parse_agent_md(&path) {
                Ok(def) => {
                    if seen.contains_key(&def.name) {
                        warn!(
                            agent_name = %def.name,
                            shadowed_by = %seen[&def.name].source_path.display(),
                            new_location = %def.source_path.display(),
                            "sub-agent name collision, keeping higher-priority version"
                        );
                    } else {
                        seen.insert(def.name.clone(), def);
                    }
                }
                Err(e) => {
                    warn!(
                        path = %path.display(),
                        error = %e,
                        "failed to parse sub-agent definition, skipping"
                    );
                }
            }
        }
    }

    seen.into_values().collect()
}

/// Parse a single `.md` file into a `SubAgentDef`.
fn parse_agent_md(path: &Path) -> Result<SubAgentDef, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("failed to read file: {e}"))?;

    let (yaml_str, body) =
        extract_frontmatter(&content).ok_or_else(|| "no YAML frontmatter found".to_string())?;

    let frontmatter: AgentFrontmatter =
        serde_yaml::from_str(yaml_str).map_err(|e| format!("invalid YAML: {e}"))?;

    let name = frontmatter
        .name
        .filter(|n| !n.is_empty())
        .ok_or_else(|| "missing required field: name".to_string())?;

    let description = frontmatter
        .description
        .filter(|d| !d.is_empty())
        .ok_or_else(|| "missing required field: description".to_string())?;

    Ok(SubAgentDef {
        name,
        description,
        system_prompt: body.to_string(),
        color: frontmatter.color,
        max_turns: frontmatter.max_turns.unwrap_or(DEFAULT_SUB_AGENT_MAX_TURNS),
        source_path: path.to_path_buf(),
    })
}

/// Build an XML catalog of available Sub-Agents for injection into system prompts.
pub fn build_sub_agent_catalog(defs: &[SubAgentDef]) -> String {
    if defs.is_empty() {
        return String::new();
    }

    let mut parts = vec!["<available-sub-agents>".to_string()];
    for def in defs {
        parts.push(format!(
            "  <agent name=\"{}\">{}</agent>",
            def.name, def.description
        ));
    }
    parts.push("</available-sub-agents>".to_string());
    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_parse_agent_md_valid() {
        let dir = tempfile::tempdir().unwrap();
        let md_path = dir.path().join("git.md");
        fs::write(
            &md_path,
            "---\nname: git\ndescription: Git operations agent\n---\n\nYou are a git expert.",
        )
        .unwrap();

        let def = parse_agent_md(&md_path).unwrap();
        assert_eq!(def.name, "git");
        assert_eq!(def.description, "Git operations agent");
        assert_eq!(def.system_prompt, "You are a git expert.");
        assert_eq!(def.max_turns, DEFAULT_SUB_AGENT_MAX_TURNS);
        assert!(def.color.is_none());
    }

    #[test]
    fn test_parse_agent_md_with_optional_fields() {
        let dir = tempfile::tempdir().unwrap();
        let md_path = dir.path().join("researcher.md");
        fs::write(
            &md_path,
            "---\nname: researcher\ndescription: Research agent\ncolor: cyan\nmaxTurns: 50\n---\n\nYou are a researcher.",
        )
        .unwrap();

        let def = parse_agent_md(&md_path).unwrap();
        assert_eq!(def.name, "researcher");
        assert_eq!(def.color.as_deref(), Some("cyan"));
        assert_eq!(def.max_turns, 50);
    }

    #[test]
    fn test_parse_agent_md_missing_name() {
        let dir = tempfile::tempdir().unwrap();
        let md_path = dir.path().join("bad.md");
        fs::write(&md_path, "---\ndescription: No name agent\n---\n\nBody.").unwrap();

        let err = parse_agent_md(&md_path).unwrap_err();
        assert!(err.contains("name"));
    }

    #[test]
    fn test_parse_agent_md_missing_description() {
        let dir = tempfile::tempdir().unwrap();
        let md_path = dir.path().join("bad.md");
        fs::write(&md_path, "---\nname: test\n---\n\nBody.").unwrap();

        let err = parse_agent_md(&md_path).unwrap_err();
        assert!(err.contains("description"));
    }

    #[test]
    fn test_parse_agent_md_claude_code_compat() {
        let dir = tempfile::tempdir().unwrap();
        let md_path = dir.path().join("compat.md");
        fs::write(
            &md_path,
            "---\nname: git\ndescription: Git agent\ntools:\n  - Bash\n  - Read\nmodel: inherit\npermissionMode: default\n---\n\nYou handle git.",
        )
        .unwrap();

        let def = parse_agent_md(&md_path).unwrap();
        assert_eq!(def.name, "git");
        assert_eq!(def.description, "Git agent");
        assert_eq!(def.system_prompt, "You handle git.");
    }

    #[test]
    fn test_discover_sub_agents_dedup() {
        let dir = tempfile::tempdir().unwrap();

        // Higher priority: .krew/agents/
        let krew_dir = dir.path().join(".krew").join("agents");
        fs::create_dir_all(&krew_dir).unwrap();
        fs::write(
            krew_dir.join("git.md"),
            "---\nname: git\ndescription: Krew git agent\n---\n\nKrew version.",
        )
        .unwrap();

        // Lower priority: .claude/agents/
        let claude_dir = dir.path().join(".claude").join("agents");
        fs::create_dir_all(&claude_dir).unwrap();
        fs::write(
            claude_dir.join("git.md"),
            "---\nname: git\ndescription: Claude git agent\n---\n\nClaude version.",
        )
        .unwrap();

        let defs = discover_sub_agents(dir.path());
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].description, "Krew git agent");
    }

    #[test]
    fn test_discover_sub_agents_multiple() {
        let dir = tempfile::tempdir().unwrap();
        let agents_dir = dir.path().join(".claude").join("agents");
        fs::create_dir_all(&agents_dir).unwrap();

        fs::write(
            agents_dir.join("git.md"),
            "---\nname: git\ndescription: Git agent\n---\n\nGit prompt.",
        )
        .unwrap();

        fs::write(
            agents_dir.join("researcher.md"),
            "---\nname: researcher\ndescription: Research agent\n---\n\nResearch prompt.",
        )
        .unwrap();

        let defs = discover_sub_agents(dir.path());
        assert_eq!(defs.len(), 2);
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"git"));
        assert!(names.contains(&"researcher"));
    }

    #[test]
    fn test_build_sub_agent_catalog_empty() {
        assert_eq!(build_sub_agent_catalog(&[]), "");
    }

    #[test]
    fn test_build_sub_agent_catalog() {
        let defs = vec![
            SubAgentDef {
                name: "git".into(),
                description: "Git operations".into(),
                system_prompt: String::new(),
                color: None,
                max_turns: 30,
                source_path: "test.md".into(),
            },
            SubAgentDef {
                name: "researcher".into(),
                description: "Research agent".into(),
                system_prompt: String::new(),
                color: None,
                max_turns: 30,
                source_path: "test.md".into(),
            },
        ];

        let catalog = build_sub_agent_catalog(&defs);
        assert!(catalog.contains("<available-sub-agents>"));
        assert!(catalog.contains("<agent name=\"git\">Git operations</agent>"));
        assert!(catalog.contains("<agent name=\"researcher\">Research agent</agent>"));
        assert!(catalog.contains("</available-sub-agents>"));
    }
}
