//! Built-in tools: read_file, write_file, edit_file, shell, glob, grep.

mod edit_file;
mod glob;
mod grep;
mod read_file;
mod shell;
mod write_file;

use std::path::PathBuf;

use crate::ToolRegistry;

pub use glob::GlobTool;
pub use grep::GrepTool;
pub use read_file::ReadFileTool;

/// Create a tool registry with all readonly built-in tools.
///
/// The `cwd` path is used as the workspace boundary for path validation.
pub fn create_readonly_registry(cwd: PathBuf) -> ToolRegistry {
    let mut registry = ToolRegistry::empty();

    let read_file = ReadFileTool::new(cwd.clone());
    registry.register(read_file.spec(), Box::new(read_file));

    let glob_tool = GlobTool::new(cwd.clone());
    registry.register(glob_tool.spec(), Box::new(glob_tool));

    let grep_tool = GrepTool::new(cwd);
    registry.register(grep_tool.spec(), Box::new(grep_tool));

    registry
}
