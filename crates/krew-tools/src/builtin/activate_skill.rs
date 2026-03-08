//! Activate skill tool: load full skill instructions into the conversation.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{ToolContext, ToolError, ToolHandler, ToolResult, ToolSpec};

/// Information about a discovered skill needed by the activation tool.
#[derive(Debug, Clone)]
pub struct SkillInfo {
    /// Absolute path to the SKILL.md file.
    pub location: PathBuf,
    /// Absolute path to the skill directory.
    pub base_dir: PathBuf,
}

/// Built-in tool for activating an Agent Skill.
///
/// Reads the SKILL.md body (stripping YAML frontmatter), wraps it in
/// identifying XML tags, and lists bundled resource files.
pub struct ActivateSkillTool {
    /// Available skills keyed by name.
    skills: HashMap<String, SkillInfo>,
    /// Track already-activated skills for deduplication.
    activated: Mutex<HashSet<String>>,
}

#[derive(Deserialize)]
struct ActivateSkillArgs {
    name: String,
}

impl ActivateSkillTool {
    pub fn new(skills: HashMap<String, SkillInfo>) -> Self {
        Self {
            skills,
            activated: Mutex::new(HashSet::new()),
        }
    }

    pub fn spec(&self) -> ToolSpec {
        // Build enum of valid skill names for the schema.
        let names: Vec<String> = self.skills.keys().cloned().collect();

        ToolSpec {
            name: "activate_skill".to_string(),
            description:
                "Activate an Agent Skill to load its specialized instructions. \
                 Call this when a task matches a skill's description from the available skills list."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The skill name to activate",
                        "enum": names,
                    }
                },
                "required": ["name"],
            }),
        }
    }

    /// Reset activation tracking (e.g. on /new session).
    pub fn reset_activated(&self) {
        if let Ok(mut set) = self.activated.lock() {
            set.clear();
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for ActivateSkillTool {
    fn name(&self) -> &str {
        "activate_skill"
    }

    fn requires_approval(&self) -> bool {
        false
    }

    fn reset_session_state(&self) {
        self.reset_activated();
    }

    fn mark_skill_activated(&self, name: &str) {
        if let Ok(mut set) = self.activated.lock() {
            set.insert(name.to_string());
        }
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        let args: ActivateSkillArgs =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        let skill_info = match self.skills.get(&args.name) {
            Some(info) => info,
            None => {
                return Ok(ToolResult {
                    content: format!(
                        "Unknown skill: \"{}\". Available skills: {}",
                        args.name,
                        self.skills.keys().cloned().collect::<Vec<_>>().join(", ")
                    ),
                    is_error: true,
                });
            }
        };

        // Deduplication check.
        {
            let mut activated = self.activated.lock().unwrap();
            if activated.contains(&args.name) {
                return Ok(ToolResult {
                    content: format!(
                        "Skill \"{}\" is already activated in this session. \
                         Its instructions are already in your context.",
                        args.name
                    ),
                    is_error: false,
                });
            }
            activated.insert(args.name.clone());
        }

        // Read SKILL.md content.
        let content = std::fs::read_to_string(&skill_info.location).map_err(|e| {
            ToolError::Execution(format!(
                "failed to read {}: {e}",
                skill_info.location.display()
            ))
        })?;

        // Strip YAML frontmatter — keep only the body.
        let body = strip_frontmatter(&content);

        // Enumerate resource files.
        let resources = enumerate_resources(&skill_info.base_dir);

        // Build response with XML wrapping.
        let base_dir_display = skill_info.base_dir.to_string_lossy().replace('\\', "/");
        let mut result = format!(
            "<skill_content name=\"{}\">\n{}\n\n\
             Skill directory: {}\n\
             Relative paths in this skill are relative to the skill directory.",
            args.name, body, base_dir_display,
        );

        if !resources.is_empty() {
            result.push_str("\n\n<skill_resources>\n");
            for res in &resources {
                result.push_str(&format!("  <file>{res}</file>\n"));
            }
            result.push_str("</skill_resources>");
        }

        result.push_str("\n</skill_content>");

        Ok(ToolResult {
            content: result,
            is_error: false,
        })
    }
}

/// Strip YAML frontmatter from SKILL.md content, returning only the body.
fn strip_frontmatter(content: &str) -> &str {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return content;
    }

    let after_open = &trimmed[3..];
    match after_open.find("\n---") {
        Some(pos) => {
            let rest = &after_open[pos + 4..];
            rest.trim_start_matches(['\r', '\n'])
        }
        None => content,
    }
}

/// List resource files in a skill directory (scripts/, references/, assets/).
fn enumerate_resources(base_dir: &Path) -> Vec<String> {
    let mut resources = Vec::new();
    let subdirs = ["scripts", "references", "assets"];

    for subdir in &subdirs {
        let path = base_dir.join(subdir);
        if !path.is_dir() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(&path) {
            for entry in entries.flatten() {
                if entry.file_type().is_ok_and(|ft| ft.is_file()) {
                    let rel_path = format!("{}/{}", subdir, entry.file_name().to_string_lossy());
                    resources.push(rel_path);
                }
            }
        }
    }

    resources.sort();
    resources
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_frontmatter() {
        let content = "---\nname: test\ndescription: A test.\n---\n\n# Instructions\nDo something.";
        assert_eq!(strip_frontmatter(content), "# Instructions\nDo something.");
    }

    #[test]
    fn test_strip_frontmatter_no_frontmatter() {
        let content = "# Just markdown\nNo frontmatter here.";
        assert_eq!(strip_frontmatter(content), content);
    }

    #[tokio::test]
    async fn test_activate_skill_unknown() {
        let tool = ActivateSkillTool::new(HashMap::new());
        let args = json!({"name": "nonexistent"});
        let result = tool.execute(args, &ToolContext::default()).await.unwrap();
        assert!(result.is_error);
        assert!(result.content.contains("Unknown skill"));
    }

    #[tokio::test]
    async fn test_activate_skill_success() {
        let dir = tempfile::tempdir().unwrap();
        let skill_md = dir.path().join("SKILL.md");
        std::fs::write(
            &skill_md,
            "---\nname: test\ndescription: Test skill.\n---\n\n# How to use\nFollow steps.",
        )
        .unwrap();

        let mut skills = HashMap::new();
        skills.insert(
            "test".to_string(),
            SkillInfo {
                location: skill_md,
                base_dir: dir.path().to_path_buf(),
            },
        );

        let tool = ActivateSkillTool::new(skills);
        let args = json!({"name": "test"});
        let result = tool.execute(args, &ToolContext::default()).await.unwrap();
        assert!(!result.is_error);
        assert!(result.content.contains("<skill_content name=\"test\">"));
        assert!(result.content.contains("# How to use"));
        assert!(result.content.contains("</skill_content>"));
    }

    #[tokio::test]
    async fn test_activate_skill_deduplication() {
        let dir = tempfile::tempdir().unwrap();
        let skill_md = dir.path().join("SKILL.md");
        std::fs::write(&skill_md, "---\nname: dup\ndescription: Dup.\n---\n\nBody.").unwrap();

        let mut skills = HashMap::new();
        skills.insert(
            "dup".to_string(),
            SkillInfo {
                location: skill_md,
                base_dir: dir.path().to_path_buf(),
            },
        );

        let tool = ActivateSkillTool::new(skills);

        // First activation should succeed.
        let r1 = tool
            .execute(json!({"name": "dup"}), &ToolContext::default())
            .await
            .unwrap();
        assert!(r1.content.contains("<skill_content"));

        // Second activation should return dedup message.
        let r2 = tool
            .execute(json!({"name": "dup"}), &ToolContext::default())
            .await
            .unwrap();
        assert!(r2.content.contains("already activated"));
    }

    #[tokio::test]
    async fn test_activate_skill_with_resources() {
        let dir = tempfile::tempdir().unwrap();
        let skill_md = dir.path().join("SKILL.md");
        std::fs::write(
            &skill_md,
            "---\nname: rich\ndescription: Rich skill.\n---\n\nInstructions.",
        )
        .unwrap();

        // Create resource files.
        let scripts_dir = dir.path().join("scripts");
        std::fs::create_dir(&scripts_dir).unwrap();
        std::fs::write(scripts_dir.join("run.sh"), "#!/bin/bash").unwrap();

        let refs_dir = dir.path().join("references");
        std::fs::create_dir(&refs_dir).unwrap();
        std::fs::write(refs_dir.join("REFERENCE.md"), "# Ref").unwrap();

        let mut skills = HashMap::new();
        skills.insert(
            "rich".to_string(),
            SkillInfo {
                location: skill_md,
                base_dir: dir.path().to_path_buf(),
            },
        );

        let tool = ActivateSkillTool::new(skills);
        let result = tool
            .execute(json!({"name": "rich"}), &ToolContext::default())
            .await
            .unwrap();
        assert!(result.content.contains("<skill_resources>"));
        assert!(result.content.contains("scripts/run.sh"));
        assert!(result.content.contains("references/REFERENCE.md"));
    }
}
