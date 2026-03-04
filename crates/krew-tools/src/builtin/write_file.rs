//! Write/create file tool.

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{ToolContext, ToolError, ToolHandler, ToolResult, ToolSpec};

/// Built-in tool for writing or creating files.
pub struct WriteFileTool {
    cwd: PathBuf,
}

#[derive(Deserialize)]
struct WriteFileArgs {
    file_path: String,
    content: String,
}

impl WriteFileTool {
    pub fn new(cwd: PathBuf) -> Self {
        Self { cwd }
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

        // For new files, we cannot canonicalize a path that doesn't exist yet.
        // We resolve the parent directory, validate it is within workspace,
        // then append the filename.
        let target = if std::path::Path::new(&args.file_path).is_absolute() {
            PathBuf::from(&args.file_path)
        } else {
            self.cwd.join(&args.file_path)
        };

        let parent = target
            .parent()
            .ok_or_else(|| ToolError::Execution("invalid file path: no parent".to_string()))?;

        // Create parent directories if needed.
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            ToolError::Execution(format!(
                "failed to create parent directories for '{}': {e}",
                target.display()
            ))
        })?;

        // Validate the parent is within workspace boundary.
        let parent_canonical = dunce::canonicalize(parent).map_err(|e| {
            ToolError::Execution(format!(
                "failed to resolve path '{}': {e}",
                parent.display()
            ))
        })?;
        let cwd_canonical = dunce::canonicalize(&self.cwd).map_err(|e| {
            ToolError::Execution(format!(
                "failed to resolve workspace path '{}': {e}",
                self.cwd.display()
            ))
        })?;
        if !parent_canonical.starts_with(&cwd_canonical) {
            return Err(ToolError::Execution(format!(
                "path '{}' is outside the workspace boundary",
                target.display()
            )));
        }

        // Build final resolved path (parent is canonical, append filename).
        let file_name = target
            .file_name()
            .ok_or_else(|| ToolError::Execution("invalid file path: no filename".to_string()))?;
        let resolved = parent_canonical.join(file_name);

        // Write content.
        tokio::fs::write(&resolved, &args.content)
            .await
            .map_err(|e| {
                ToolError::Execution(format!(
                    "failed to write file '{}': {e}",
                    resolved.display()
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
