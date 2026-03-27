//! Sub-Agent definition types.

use std::path::PathBuf;

/// A discovered Sub-Agent definition parsed from a Markdown file.
#[derive(Debug, Clone)]
pub struct SubAgentDef {
    /// Sub-Agent name (from YAML frontmatter `name` field).
    pub name: String,
    /// Short description (from YAML frontmatter `description` field).
    pub description: String,
    /// System prompt (Markdown body after frontmatter).
    pub system_prompt: String,
    /// Optional TUI display color.
    pub color: Option<String>,
    /// Maximum agent loop turns (default 30).
    pub max_turns: u32,
    /// Absolute path to the source `.md` file.
    pub source_path: PathBuf,
}

/// Default maximum turns for a Sub-Agent loop.
pub const DEFAULT_SUB_AGENT_MAX_TURNS: u32 = 30;
