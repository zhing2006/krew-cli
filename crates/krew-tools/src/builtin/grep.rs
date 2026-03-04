//! File content search (grep) tool.
//!
//! Pure Rust implementation using `regex` + `walkdir` for searching
//! file contents. No external commands required.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use globset::{Glob as GlobPattern, GlobSetBuilder};
use serde::Deserialize;
use serde_json::{Value, json};
use walkdir::WalkDir;

use crate::{
    ToolContext, ToolError, ToolHandler, ToolResult, ToolSpec, check_binary, check_file_size,
    validate_path,
};

const DEFAULT_LIMIT: usize = 100;
const MAX_LIMIT: usize = 2000;
const MAX_LINE_LENGTH: usize = 500;

/// Built-in tool for searching file contents with regex.
pub struct GrepTool {
    cwd: PathBuf,
}

#[derive(Deserialize)]
struct GrepArgs {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    include: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

impl GrepTool {
    pub fn new(cwd: PathBuf) -> Self {
        Self { cwd }
    }

    pub fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "grep".to_string(),
            description: "Search file contents using regular expressions. Returns matching lines \
                          with file path, line number, and context. Results are grouped by file."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Regular expression pattern to search for"
                    },
                    "path": {
                        "type": "string",
                        "description": "Directory or file to search in (default: workspace root)"
                    },
                    "include": {
                        "type": "string",
                        "description": "Glob to filter which files are searched (e.g. \"*.rs\", \"*.{ts,tsx}\")"
                    },
                    "limit": {
                        "type": "number",
                        "description": "Maximum number of matching lines to return (default: 100)"
                    }
                },
                "required": ["pattern"],
                "additionalProperties": false
            }),
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn requires_approval(&self) -> bool {
        false
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        let args: GrepArgs =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        let pattern_str = args.pattern.trim();
        if pattern_str.is_empty() {
            return Err(ToolError::InvalidArgs(
                "pattern must not be empty".to_string(),
            ));
        }

        let limit = args.limit.clamp(1, MAX_LIMIT);

        let regex = regex::Regex::new(pattern_str)
            .map_err(|e| ToolError::InvalidArgs(format!("invalid regex pattern: {e}")))?;

        let search_path = if let Some(ref path) = args.path {
            validate_path(path, &self.cwd)?
        } else {
            self.cwd.clone()
        };

        // Build include filter if specified.
        let include_filter = if let Some(ref include) = args.include {
            let glob = GlobPattern::new(include.trim())
                .map_err(|e| ToolError::InvalidArgs(format!("invalid include glob: {e}")))?;
            let mut builder = GlobSetBuilder::new();
            builder.add(glob);
            Some(
                builder
                    .build()
                    .map_err(|e| ToolError::InvalidArgs(format!("failed to compile glob: {e}")))?,
            )
        } else {
            None
        };

        let cwd_canonical = dunce::canonicalize(&self.cwd)
            .map_err(|e| ToolError::Execution(format!("failed to resolve workspace path: {e}")))?;

        // Collect files to search.
        let files: Vec<PathBuf> = if search_path.is_file() {
            vec![search_path.clone()]
        } else {
            let mut file_list = Vec::new();
            for entry in WalkDir::new(&search_path)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| {
                    let name = e.file_name().to_string_lossy();
                    !name.starts_with('.') || e.depth() == 0
                })
            {
                let entry = match entry {
                    Ok(e) => e,
                    Err(_) => continue,
                };

                if !entry.file_type().is_file() {
                    continue;
                }

                // Apply include filter.
                if let Some(ref filter) = include_filter {
                    let file_name = entry.file_name().to_string_lossy();
                    if !filter.is_match(file_name.as_ref()) {
                        continue;
                    }
                }

                let full_path = match dunce::canonicalize(entry.path()) {
                    Ok(p) => p,
                    Err(_) => continue,
                };

                // Ensure within workspace boundary.
                if !full_path.starts_with(&cwd_canonical) {
                    continue;
                }

                file_list.push(full_path);
            }
            file_list
        };

        // Search through files.
        let mut results = Vec::new();
        let mut match_count = 0;

        'outer: for file_path in &files {
            // Skip oversized or binary files.
            if check_file_size(file_path).is_some() || check_binary(file_path).is_some() {
                continue;
            }

            let file = match std::fs::File::open(file_path) {
                Ok(f) => f,
                Err(_) => continue,
            };

            let reader = BufReader::new(file);
            let relative_path = file_path.strip_prefix(&cwd_canonical).unwrap_or(file_path);

            for (line_idx, line) in reader.lines().enumerate() {
                let line = match line {
                    Ok(l) => l,
                    Err(_) => continue,
                };

                if regex.is_match(&line) {
                    let line_num = line_idx + 1;
                    let display_line = if line.len() > MAX_LINE_LENGTH {
                        format!("{}...", &line[..MAX_LINE_LENGTH])
                    } else {
                        line
                    };

                    results.push(format!(
                        "{}:L{}: {}",
                        relative_path.display(),
                        line_num,
                        display_line
                    ));

                    match_count += 1;
                    if match_count >= limit {
                        break 'outer;
                    }
                }
            }
        }

        if results.is_empty() {
            return Ok(ToolResult {
                content: "No matches found.".to_string(),
                is_error: false,
            });
        }

        let content = results.join("\n");
        Ok(ToolResult {
            content: format!("{content}\n\n({match_count} matches)"),
            is_error: false,
        })
    }
}
