//! Read file contents tool.

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::{
    ImageContent, MAX_IMAGE_SIZE, ToolContext, ToolError, ToolHandler, ToolResult, ToolSpec,
    check_binary, check_file_size, validate_path,
};

const MAX_LINE_LENGTH: usize = 2000;
const DEFAULT_OFFSET: usize = 1;
const DEFAULT_LIMIT: usize = 2000;

/// Built-in tool for reading file contents with line numbers.
pub struct ReadFileTool {
    cwd: PathBuf,
    restrict_workspace: bool,
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

/// Return the MIME type for supported image extensions, or None.
fn image_media_type(path: &std::path::Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some("image/png".to_string()),
        "jpg" | "jpeg" => Some("image/jpeg".to_string()),
        "gif" => Some("image/gif".to_string()),
        "webp" => Some("image/webp".to_string()),
        _ => None,
    }
}

impl ReadFileTool {
    pub fn new(cwd: PathBuf, restrict_workspace: bool) -> Self {
        Self {
            cwd,
            restrict_workspace,
        }
    }

    /// Read an image file and return its bytes as ImageContent.
    async fn read_image(
        &self,
        path: &std::path::Path,
        media_type: &str,
    ) -> Result<ToolResult, ToolError> {
        let size = std::fs::metadata(path)
            .map_err(|e| {
                ToolError::Execution(format!(
                    "failed to read metadata for '{}': {e}",
                    path.display()
                ))
            })?
            .len();

        if size > MAX_IMAGE_SIZE {
            let size_mb = size / (1024 * 1024);
            return Ok(ToolResult {
                content: format!(
                    "Image file '{}' is too large ({size_mb} MB). Maximum allowed size for images is {} MB.",
                    path.display(),
                    MAX_IMAGE_SIZE / (1024 * 1024)
                ),
                is_error: true,
                images: vec![],
            });
        }

        let data = tokio::fs::read(path).await.map_err(|e| {
            ToolError::Execution(format!("failed to read image '{}': {e}", path.display()))
        })?;

        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        Ok(ToolResult {
            content: format!("[Image: {filename}]"),
            images: vec![ImageContent {
                data,
                media_type: media_type.to_string(),
                filename: Some(filename.clone()),
            }],
            is_error: false,
        })
    }

    pub fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "read_file".to_string(),
            description: "Read file contents. Output includes line numbers prefixed with 'L'. \
                          Use offset and limit for partial reads on large files. \
                          Also supports reading image files (png, jpg, jpeg, gif, webp) — \
                          use this tool to view image contents."
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

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<ToolResult, ToolError> {
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

        let path = validate_path(&args.file_path, &self.cwd, self.restrict_workspace)?;

        // Check for image files before binary detection (images are binary).
        if let Some(media_type) = image_media_type(&path) {
            return self.read_image(&path, &media_type).await;
        }

        if let Some(result) = check_file_size(&path) {
            return Ok(result);
        }
        if let Some(result) = check_binary(&path) {
            return Ok(result);
        }

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
                images: vec![],
            });
        }

        let num_lines = collected.len();
        let content = collected.join("\n");

        Ok(ToolResult {
            content: format!("{content}\n\n({num_lines} lines)"),
            is_error: false,
            images: vec![],
        })
    }
}
