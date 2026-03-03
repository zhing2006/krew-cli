//! Read file contents tool.

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::{ToolError, ToolHandler, ToolResult, ToolSpec, validate_path};

const MAX_LINE_LENGTH: usize = 2000;
const DEFAULT_OFFSET: usize = 1;
const DEFAULT_LIMIT: usize = 2000;

/// Built-in tool for reading file contents with line numbers.
pub struct ReadFileTool {
    cwd: PathBuf,
}

#[derive(Deserialize)]
struct ReadFileArgs {
    file_path: String,
    #[serde(default = "default_offset")]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_offset() -> usize {
    DEFAULT_OFFSET
}
fn default_limit() -> usize {
    DEFAULT_LIMIT
}

impl ReadFileTool {
    pub fn new(cwd: PathBuf) -> Self {
        Self { cwd }
    }

    pub fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "read_file".to_string(),
            description: "Read file contents. Output includes line numbers prefixed with 'L'. \
                          Use offset and limit for partial reads on large files."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file (absolute or relative to workspace)"
                    },
                    "offset": {
                        "type": "number",
                        "description": "1-indexed line number to start reading from (default: 1)"
                    },
                    "limit": {
                        "type": "number",
                        "description": "Maximum number of lines to return (default: 2000)"
                    }
                },
                "required": ["file_path"],
                "additionalProperties": false
            }),
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn requires_approval(&self) -> bool {
        false
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let args: ReadFileArgs =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        if args.offset == 0 {
            return Err(ToolError::InvalidArgs(
                "offset must be a 1-indexed line number".to_string(),
            ));
        }
        if args.limit == 0 {
            return Err(ToolError::InvalidArgs(
                "limit must be greater than zero".to_string(),
            ));
        }

        let path = validate_path(&args.file_path, &self.cwd)?;

        let file = tokio::fs::File::open(&path).await.map_err(|e| {
            ToolError::Execution(format!("failed to open file '{}': {e}", path.display()))
        })?;

        let mut reader = BufReader::new(file);
        let mut collected = Vec::new();
        let mut line_num = 0usize;
        let mut buffer = Vec::new();

        loop {
            buffer.clear();
            let bytes_read = reader
                .read_until(b'\n', &mut buffer)
                .await
                .map_err(|e| ToolError::Execution(format!("failed to read file: {e}")))?;

            if bytes_read == 0 {
                break;
            }

            // Strip trailing newline/CRLF.
            if buffer.last() == Some(&b'\n') {
                buffer.pop();
                if buffer.last() == Some(&b'\r') {
                    buffer.pop();
                }
            }

            line_num += 1;

            if line_num < args.offset {
                continue;
            }
            if collected.len() >= args.limit {
                break;
            }

            let line_text = String::from_utf8_lossy(&buffer);
            let display = if line_text.len() > MAX_LINE_LENGTH {
                &line_text[..MAX_LINE_LENGTH]
            } else {
                &line_text
            };
            collected.push(format!("L{line_num}: {display}"));
        }

        if line_num < args.offset {
            return Ok(ToolResult {
                content: format!(
                    "offset {offset} exceeds file length ({line_num} lines)",
                    offset = args.offset
                ),
                is_error: true,
            });
        }

        let num_lines = collected.len();
        let content = collected.join("\n");

        Ok(ToolResult {
            content: format!("{content}\n\n({num_lines} lines)"),
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
        let file_path = dir.path().join("test.txt");
        let mut f = std::fs::File::create(&file_path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        (dir, file_path)
    }

    #[tokio::test]
    async fn reads_full_file() {
        let (dir, file_path) = setup_test_file("alpha\nbeta\ngamma\n");
        let tool = ReadFileTool::new(dir.path().to_path_buf());

        let result = tool
            .execute(json!({ "file_path": file_path.to_str().unwrap() }))
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(result.content.contains("L1: alpha"));
        assert!(result.content.contains("L2: beta"));
        assert!(result.content.contains("L3: gamma"));
        assert!(result.content.contains("(3 lines)"));
    }

    #[tokio::test]
    async fn reads_with_offset_and_limit() {
        let (dir, file_path) = setup_test_file("first\nsecond\nthird\nfourth\n");
        let tool = ReadFileTool::new(dir.path().to_path_buf());

        let result = tool
            .execute(json!({
                "file_path": file_path.to_str().unwrap(),
                "offset": 2,
                "limit": 2
            }))
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(result.content.contains("L2: second"));
        assert!(result.content.contains("L3: third"));
        assert!(!result.content.contains("L1:"));
        assert!(!result.content.contains("L4:"));
    }

    #[tokio::test]
    async fn offset_exceeds_file_length() {
        let (dir, file_path) = setup_test_file("only\n");
        let tool = ReadFileTool::new(dir.path().to_path_buf());

        let result = tool
            .execute(json!({
                "file_path": file_path.to_str().unwrap(),
                "offset": 10
            }))
            .await
            .unwrap();

        assert!(result.is_error);
        assert!(result.content.contains("exceeds file length"));
    }

    #[tokio::test]
    async fn rejects_path_outside_workspace() {
        let dir = TempDir::new().unwrap();
        let tool = ReadFileTool::new(dir.path().to_path_buf());

        let result = tool.execute(json!({ "file_path": "/etc/passwd" })).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn handles_crlf_line_endings() {
        let (dir, file_path) = setup_test_file("one\r\ntwo\r\n");
        let tool = ReadFileTool::new(dir.path().to_path_buf());

        let result = tool
            .execute(json!({ "file_path": file_path.to_str().unwrap() }))
            .await
            .unwrap();

        assert!(result.content.contains("L1: one"));
        assert!(result.content.contains("L2: two"));
    }

    #[tokio::test]
    async fn invalid_offset_zero() {
        let (dir, file_path) = setup_test_file("test\n");
        let tool = ReadFileTool::new(dir.path().to_path_buf());

        let result = tool
            .execute(json!({
                "file_path": file_path.to_str().unwrap(),
                "offset": 0
            }))
            .await;

        assert!(result.is_err());
    }
}
