//! Write/create file tool.

use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{ToolContext, ToolError, ToolHandler, ToolResult, ToolSpec};

/// Built-in tool for writing or creating files.
pub struct WriteFileTool {
    cwd: PathBuf,
    restrict_workspace: bool,
}

#[derive(Deserialize)]
struct WriteFileArgs {
    file_path: String,
    content: String,
}

impl WriteFileTool {
    pub fn new(cwd: PathBuf, restrict_workspace: bool) -> Self {
        Self {
            cwd,
            restrict_workspace,
        }
    }

    pub fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "write_file".to_string(),
            description: "Create or overwrite a file with the given content. \
                          Parent directories are created automatically."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file (absolute or relative to workspace)"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file"
                    }
                },
                "required": ["file_path", "content"],
                "additionalProperties": false
            }),
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn requires_approval(&self) -> bool {
        true
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        let args: WriteFileArgs =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        // Canonicalize workspace root first — this path must already exist.
        let cwd_canonical = dunce::canonicalize(&self.cwd).map_err(|e| {
            ToolError::Execution(format!(
                "failed to resolve workspace path '{}': {e}",
                self.cwd.display()
            ))
        })?;

        // Build target path. For relative paths, join to the canonical cwd so
        // the prefix matches exactly for the boundary check below.
        let target = if Path::new(&args.file_path).is_absolute() {
            PathBuf::from(&args.file_path)
        } else {
            cwd_canonical.join(&args.file_path)
        };

        // Normalize target path purely (resolve `.` and `..` without touching
        // the filesystem) so boundary checks work before any side effects.
        let normalized = normalize_path(&target);

        // Validate the normalized target is within the workspace boundary
        // BEFORE creating any directories or files.
        if self.restrict_workspace && !normalized.starts_with(&cwd_canonical) {
            return Err(ToolError::Execution(format!(
                "path '{}' is outside the workspace boundary",
                target.display()
            )));
        }

        // Boundary check passed — safe to create parent directories now.
        let parent = normalized
            .parent()
            .ok_or_else(|| ToolError::Execution("invalid file path: no parent".to_string()))?;

        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            ToolError::Execution(format!(
                "failed to create parent directories for '{}': {e}",
                target.display()
            ))
        })?;

        // Write content.
        tokio::fs::write(&normalized, &args.content)
            .await
            .map_err(|e| {
                ToolError::Execution(format!(
                    "failed to write file '{}': {e}",
                    normalized.display()
                ))
            })?;

        let line_count = args.content.lines().count();
        let byte_count = args.content.len();

        Ok(ToolResult {
            content: format!(
                "Successfully wrote to '{}' ({line_count} lines, {byte_count} bytes)",
                args.file_path
            ),
            is_error: false,
        })
    }
}

/// Normalize a path by resolving `.` and `..` components purely at the path
/// level, without touching the filesystem. This allows boundary checks to
/// work even when parent directories do not yet exist.
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                components.pop();
            }
            Component::CurDir => {}
            c => {
                components.push(c);
            }
        }
    }
    components.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_path_outside_workspace() {
        let tmp = std::env::temp_dir().join("krew_write_test");
        let _ = tokio::fs::create_dir_all(&tmp).await;
        let tool = WriteFileTool::new(tmp.clone(), true);

        let args = serde_json::json!({
            "file_path": "../../../outside/evil.txt",
            "content": "bad"
        });
        let ctx = ToolContext::default();
        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err());

        // Verify no directories were created outside workspace.
        let outside = tmp.join("../../../outside");
        assert!(!outside.exists());

        let _ = tokio::fs::remove_dir_all(&tmp).await;
    }

    #[tokio::test]
    async fn allows_path_inside_workspace() {
        let tmp = std::env::temp_dir().join("krew_write_test_ok");
        let _ = tokio::fs::create_dir_all(&tmp).await;
        let tool = WriteFileTool::new(tmp.clone(), true);

        let args = serde_json::json!({
            "file_path": "sub/dir/test.txt",
            "content": "hello"
        });
        let ctx = ToolContext::default();
        let result = tool.execute(args, &ctx).await;
        assert!(result.is_ok());

        let _ = tokio::fs::remove_dir_all(&tmp).await;
    }
}
