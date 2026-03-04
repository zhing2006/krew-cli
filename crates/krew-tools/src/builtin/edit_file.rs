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
}

#[derive(Deserialize)]
struct EditFileArgs {
    file_path: String,
    old_string: String,
    new_string: String,
}

impl EditFileTool {
    pub fn new(cwd: PathBuf) -> Self {
        Self { cwd }
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

        let path = validate_path(&args.file_path, &self.cwd)?;

        if let Some(result) = check_file_size(&path) {
            return Ok(result);
        }
        if let Some(result) = check_binary(&path) {
            return Ok(result);
        }

        let original = tokio::fs::read_to_string(&path).await.map_err(|e| {
            ToolError::Execution(format!("failed to read file '{}': {e}", path.display()))
        })?;

        // Verify old_string appears exactly once.
        let match_count = original.matches(&args.old_string).count();
        if match_count == 0 {
            return Ok(ToolResult {
                content: format!(
                    "old_string not found in '{}'. Make sure the string matches exactly, \
                     including whitespace and indentation.",
                    args.file_path
                ),
                is_error: true,
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
            });
        }

        // Perform replacement.
        let modified = original.replacen(&args.old_string, &args.new_string, 1);

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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn setup_test_file(content: &str) -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.rs");
        let mut f = std::fs::File::create(&file_path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        (dir, file_path)
    }

    #[tokio::test]
    async fn single_replacement() {
        let (dir, file_path) = setup_test_file("fn main() {\n    println!(\"hello\");\n}\n");
        let tool = EditFileTool::new(dir.path().to_path_buf());

        let result = tool
            .execute(
                json!({
                    "file_path": file_path.to_str().unwrap(),
                    "old_string": "println!(\"hello\")",
                    "new_string": "println!(\"world\")"
                }),
                &ToolContext::default(),
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(result.content.contains("-    println!(\"hello\")"));
        assert!(result.content.contains("+    println!(\"world\")"));

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("println!(\"world\")"));
        assert!(!content.contains("println!(\"hello\")"));
    }

    #[tokio::test]
    async fn old_string_not_found() {
        let (dir, file_path) = setup_test_file("fn main() {}\n");
        let tool = EditFileTool::new(dir.path().to_path_buf());

        let result = tool
            .execute(
                json!({
                    "file_path": file_path.to_str().unwrap(),
                    "old_string": "nonexistent text",
                    "new_string": "replacement"
                }),
                &ToolContext::default(),
            )
            .await
            .unwrap();

        assert!(result.is_error);
        assert!(result.content.contains("not found"));
    }

    #[tokio::test]
    async fn multiple_matches_error() {
        let (dir, file_path) = setup_test_file("aaa\nbbb\naaa\n");
        let tool = EditFileTool::new(dir.path().to_path_buf());

        let result = tool
            .execute(
                json!({
                    "file_path": file_path.to_str().unwrap(),
                    "old_string": "aaa",
                    "new_string": "ccc"
                }),
                &ToolContext::default(),
            )
            .await
            .unwrap();

        assert!(result.is_error);
        assert!(result.content.contains("2 times"));
    }

    #[tokio::test]
    async fn file_not_found() {
        let dir = TempDir::new().unwrap();
        let tool = EditFileTool::new(dir.path().to_path_buf());

        let result = tool
            .execute(
                json!({
                    "file_path": "nonexistent.rs",
                    "old_string": "a",
                    "new_string": "b"
                }),
                &ToolContext::default(),
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn rejects_path_outside_workspace() {
        let dir = TempDir::new().unwrap();
        let tool = EditFileTool::new(dir.path().to_path_buf());

        let result = tool
            .execute(
                json!({
                    "file_path": "/etc/passwd",
                    "old_string": "root",
                    "new_string": "hacked"
                }),
                &ToolContext::default(),
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn diff_output_format() {
        let (dir, file_path) = setup_test_file("line1\nline2\nline3\n");
        let tool = EditFileTool::new(dir.path().to_path_buf());

        let result = tool
            .execute(
                json!({
                    "file_path": file_path.to_str().unwrap(),
                    "old_string": "line2",
                    "new_string": "modified"
                }),
                &ToolContext::default(),
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        // Should contain unified diff markers.
        assert!(result.content.contains("@@"));
        assert!(result.content.contains("-line2"));
        assert!(result.content.contains("+modified"));
    }
}
