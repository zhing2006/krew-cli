//! File pattern matching (glob) tool.

use std::path::PathBuf;

use globset::{Glob as GlobPattern, GlobSetBuilder};
use serde::Deserialize;
use serde_json::{Value, json};
use walkdir::WalkDir;

use crate::{ToolContext, ToolError, ToolHandler, ToolResult, ToolSpec, validate_path};

const DEFAULT_LIMIT: usize = 200;
const MAX_LIMIT: usize = 2000;

/// Built-in tool for finding files by glob pattern.
pub struct GlobTool {
    cwd: PathBuf,
    restrict_workspace: bool,
}

#[derive(Deserialize)]
struct GlobArgs {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

impl GlobTool {
    pub fn new(cwd: PathBuf, restrict_workspace: bool) -> Self {
        Self {
            cwd,
            restrict_workspace,
        }
    }

    pub fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "glob".to_string(),
            description: "Find files matching a glob pattern (e.g. \"**/*.rs\", \"src/**/*.ts\"). \
                          Returns matching file paths sorted by modification time (newest first)."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Glob pattern to match files (e.g. \"**/*.rs\", \"src/*.ts\")"
                    },
                    "path": {
                        "type": "string",
                        "description": "Directory to search in (default: workspace root)"
                    },
                    "limit": {
                        "type": "number",
                        "description": "Maximum number of results to return (default: 200)"
                    }
                },
                "required": ["pattern"],
                "additionalProperties": false
            }),
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn requires_approval(&self) -> bool {
        false
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        let args: GlobArgs =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        let pattern = args.pattern.trim();
        if pattern.is_empty() {
            return Err(ToolError::InvalidArgs(
                "pattern must not be empty".to_string(),
            ));
        }

        let limit = args.limit.clamp(1, MAX_LIMIT);

        let search_dir = if let Some(ref path) = args.path {
            validate_path(path, &self.cwd, self.restrict_workspace)?
        } else {
            self.cwd.clone()
        };

        if !search_dir.is_dir() {
            return Ok(ToolResult {
                content: format!("'{}' is not a directory", search_dir.display()),
                is_error: true,
            });
        }

        // Build glob matcher.
        let glob = GlobPattern::new(pattern)
            .map_err(|e| ToolError::InvalidArgs(format!("invalid glob pattern: {e}")))?;
        let mut builder = GlobSetBuilder::new();
        builder.add(glob);
        let glob_set = builder
            .build()
            .map_err(|e| ToolError::InvalidArgs(format!("failed to compile glob: {e}")))?;

        // Walk directory and collect matches.
        let cwd_canonical = if self.restrict_workspace {
            Some(dunce::canonicalize(&self.cwd).map_err(|e| {
                ToolError::Execution(format!("failed to resolve workspace path: {e}"))
            })?)
        } else {
            None
        };

        let mut matches: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

        for entry in WalkDir::new(&search_dir)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                // Skip hidden directories (except the search root).
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

            // Ensure within workspace boundary when restricted.
            if let Some(ref cwd_canon) = cwd_canonical {
                let full_path = match dunce::canonicalize(entry.path()) {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                if !full_path.starts_with(cwd_canon) {
                    continue;
                }
            }

            // Match against relative path from search_dir.
            let relative = match entry.path().strip_prefix(&search_dir) {
                Ok(r) => r,
                Err(_) => continue,
            };

            if glob_set.is_match(relative) {
                let modified = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                matches.push((relative.to_path_buf(), modified));
            }
        }

        // Sort by modification time (newest first).
        matches.sort_by(|a, b| b.1.cmp(&a.1));
        matches.truncate(limit);

        if matches.is_empty() {
            return Ok(ToolResult {
                content: "No files matched the pattern.".to_string(),
                is_error: false,
            });
        }

        let file_count = matches.len();
        let content: Vec<String> = matches
            .iter()
            .map(|(p, _)| p.display().to_string())
            .collect();

        Ok(ToolResult {
            content: format!("{}\n\n({file_count} files)", content.join("\n")),
            is_error: false,
        })
    }
}
