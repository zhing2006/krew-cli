//! Search-and-replace file editing tool.

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{Value, json};
use similar::TextDiff;

use crate::{
    ToolContext, ToolError, ToolHandler, ToolResult, ToolSpec, check_binary, check_file_size,
    validate_path,
};

/// Built-in tool for editing files via search-and-replace.
pub struct EditFileTool {
    cwd: PathBuf,
    restrict_workspace: bool,
}

#[derive(Deserialize)]
struct EditFileArgs {
    file_path: String,
    old_string: String,
    new_string: String,
}

impl EditFileTool {
    pub fn new(cwd: PathBuf, restrict_workspace: bool) -> Self {
        Self {
            cwd,
            restrict_workspace,
        }
    }

    pub fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "edit_file".to_string(),
            description: "Edit a file by replacing an exact string match. \
                          The old_string must appear exactly once in the file. \
                          Returns a unified diff of the changes."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file (absolute or relative to workspace)"
                    },
                    "old_string": {
                        "type": "string",
                        "description": "Exact string to find in the file (must appear exactly once)"
                    },
                    "new_string": {
                        "type": "string",
                        "description": "Replacement string"
                    }
                },
                "required": ["file_path", "old_string", "new_string"],
                "additionalProperties": false
            }),
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn requires_approval(&self) -> bool {
        true
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        let args: EditFileArgs =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        let path = validate_path(&args.file_path, &self.cwd, self.restrict_workspace)?;

        if let Some(result) = check_file_size(&path) {
            return Ok(result);
        }
        if let Some(result) = check_binary(&path) {
            return Ok(result);
        }

        let original = tokio::fs::read_to_string(&path).await.map_err(|e| {
            ToolError::Execution(format!("failed to read file '{}': {e}", path.display()))
        })?;

        // Normalize old_string/new_string line endings to match the file.
        // LLMs always send \n in JSON, but the file may use \r\n (CRLF).
        let uses_crlf = original.contains("\r\n");
        let old_string = if uses_crlf {
            args.old_string.replace("\r\n", "\n").replace('\n', "\r\n")
        } else {
            args.old_string.replace("\r\n", "\n")
        };
        let new_string = if uses_crlf {
            args.new_string.replace("\r\n", "\n").replace('\n', "\r\n")
        } else {
            args.new_string.replace("\r\n", "\n")
        };

        // Verify old_string appears exactly once.
        let match_count = original.matches(&old_string).count();
        if match_count == 0 {
            return Ok(ToolResult {
                content: format!(
                    "old_string not found in '{}'. Make sure the string matches exactly, \
                     including whitespace and indentation.",
                    args.file_path
                ),
                is_error: true,
                images: vec![],
            });
        }
        if match_count > 1 {
            return Ok(ToolResult {
                content: format!(
                    "old_string found {match_count} times in '{}'. \
                     Provide more surrounding context to make the match unique.",
                    args.file_path
                ),
                is_error: true,
                images: vec![],
            });
        }

        // Perform replacement.
        let modified = original.replacen(&old_string, &new_string, 1);

        // Generate unified diff.
        let diff = TextDiff::from_lines(&original, &modified);
        let unified = diff
            .unified_diff()
            .context_radius(3)
            .header(&args.file_path, &args.file_path)
            .to_string();

        // Write modified content back.
        tokio::fs::write(&path, &modified).await.map_err(|e| {
            ToolError::Execution(format!("failed to write file '{}': {e}", path.display()))
        })?;

        Ok(ToolResult {
            content: unified,
            is_error: false,
            images: vec![],
        })
    }
}
