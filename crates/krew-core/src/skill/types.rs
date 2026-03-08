//! Agent Skills types.

use std::collections::HashMap;
use std::path::PathBuf;

/// A discovered skill parsed from a SKILL.md file.
#[derive(Debug, Clone)]
pub struct SkillRecord {
    /// Skill name from YAML frontmatter.
    pub name: String,
    /// Skill description from YAML frontmatter.
    pub description: String,
    /// Absolute path to the SKILL.md file.
    pub location: PathBuf,
    /// Absolute path to the skill directory (parent of SKILL.md).
    pub base_dir: PathBuf,
    /// Optional compatibility notes.
    pub compatibility: Option<String>,
    /// Optional metadata key-value pairs.
    pub metadata: Option<HashMap<String, String>>,
}

/// Errors that can occur during skill discovery and parsing.
#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    /// SKILL.md file could not be read.
    #[error("failed to read SKILL.md: {0}")]
    Io(#[from] std::io::Error),
    /// YAML frontmatter is missing or unparseable.
    #[error("invalid YAML frontmatter: {0}")]
    InvalidFrontmatter(String),
    /// Required field is missing.
    #[error("missing required field: {0}")]
    MissingField(String),
}
