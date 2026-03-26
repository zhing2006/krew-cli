//! Built-in tools: read_file, write_file, edit_file, shell, glob, grep, fetch_url, activate_skill.

pub mod activate_skill;
mod edit_file;
pub mod fetch_url;
mod glob;
mod grep;
mod read_file;
mod shell;
pub mod shell_parse;
mod write_file;

use std::collections::HashMap;
use std::path::PathBuf;

use crate::ToolRegistry;

pub use activate_skill::{ActivateSkillTool, SkillInfo};
pub use edit_file::EditFileTool;
pub use fetch_url::FetchUrlTool;
pub use glob::GlobTool;
pub use grep::GrepTool;
pub use read_file::ReadFileTool;
pub use shell::ShellTool;
pub use shell_parse::{extract_command_prefixes, matches_allowlist_entry};
pub use write_file::WriteFileTool;

/// Create a tool registry with all readonly built-in tools.
///
/// The `cwd` path is used as the workspace boundary for path validation.
/// When `restrict_workspace` is `false`, file tools can access any path.
pub fn create_readonly_registry(cwd: PathBuf, restrict_workspace: bool) -> ToolRegistry {
    let mut registry = ToolRegistry::empty();

    let read_file = ReadFileTool::new(cwd.clone(), restrict_workspace);
    registry.register(read_file.spec(), Box::new(read_file));

    let glob_tool = GlobTool::new(cwd.clone(), restrict_workspace);
    registry.register(glob_tool.spec(), Box::new(glob_tool));

    let grep_tool = GrepTool::new(cwd, restrict_workspace);
    registry.register(grep_tool.spec(), Box::new(grep_tool));

    registry
}

/// Create a tool registry with all built-in tools (read + write + shell + fetch).
///
/// The `cwd` path is used as the workspace boundary for path validation.
/// When `restrict_workspace` is `false`, file tools can access any path.
/// When `skills` is non-empty, an `activate_skill` tool is also registered.
pub fn create_full_registry(
    cwd: PathBuf,
    restrict_workspace: bool,
    skills: HashMap<String, SkillInfo>,
) -> ToolRegistry {
    let mut registry = ToolRegistry::empty();

    // Readonly tools.
    let read_file = ReadFileTool::new(cwd.clone(), restrict_workspace);
    registry.register(read_file.spec(), Box::new(read_file));

    let glob_tool = GlobTool::new(cwd.clone(), restrict_workspace);
    registry.register(glob_tool.spec(), Box::new(glob_tool));

    let grep_tool = GrepTool::new(cwd.clone(), restrict_workspace);
    registry.register(grep_tool.spec(), Box::new(grep_tool));

    // Write tools.
    let write_file = WriteFileTool::new(cwd.clone(), restrict_workspace);
    registry.register(write_file.spec(), Box::new(write_file));

    let edit_file = EditFileTool::new(cwd.clone(), restrict_workspace);
    registry.register(edit_file.spec(), Box::new(edit_file));

    // Shell tool.
    let shell_tool = ShellTool::new(cwd);
    registry.register(shell_tool.spec(), Box::new(shell_tool));

    // Fetch URL tool.
    let fetch_tool = FetchUrlTool::new();
    registry.register(fetch_tool.spec(), Box::new(fetch_tool));

    // Activate skill tool (only when skills are available).
    if !skills.is_empty() {
        let skill_tool = ActivateSkillTool::new(skills);
        registry.register(skill_tool.spec(), Box::new(skill_tool));
    }

    registry
}
