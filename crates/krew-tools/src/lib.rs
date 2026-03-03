pub mod builtin;
pub mod mcp;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// Maximum file size allowed for reading (100 MB).
pub const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;
/// Number of bytes to probe for binary detection.
const BINARY_PROBE_SIZE: usize = 8192;

/// Result returned by a tool after execution.
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// Output content from the tool.
    pub content: String,
    /// Whether the tool execution resulted in an error.
    pub is_error: bool,
}

/// Errors that can occur during tool execution.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("tool execution failed: {0}")]
    Execution(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid arguments: {0}")]
    InvalidArgs(String),
}

/// Tool specification sent to LLM providers (JSON Schema).
///
/// This is the "what the LLM sees" half of the tool system. It contains
/// the tool name, description, and parameter schema that get included in
/// the LLM API request.
#[derive(Debug, Clone)]
pub struct ToolSpec {
    /// Tool name (must match the registered handler).
    pub name: String,
    /// Human-readable description of what the tool does.
    pub description: String,
    /// JSON Schema describing the tool's input parameters.
    pub parameters: Value,
}

/// Trait for tool execution logic.
///
/// This is the "how it runs" half of the tool system. Implementations
/// contain the actual logic for executing a tool call.
#[async_trait::async_trait]
pub trait ToolHandler: Send + Sync {
    /// Unique tool name (must match the corresponding ToolSpec).
    fn name(&self) -> &str;
    /// Whether this tool requires user approval before execution.
    fn requires_approval(&self) -> bool;
    /// Execute the tool with the given arguments.
    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError>;
}

/// Registry that pairs tool specs (for LLM) with handlers (for execution).
pub struct ToolRegistry {
    specs: Vec<ToolSpec>,
    handlers: HashMap<String, Box<dyn ToolHandler>>,
}

impl ToolRegistry {
    /// Create an empty registry with no tools.
    pub fn empty() -> Self {
        Self {
            specs: Vec::new(),
            handlers: HashMap::new(),
        }
    }

    /// Register a tool spec and its corresponding handler.
    pub fn register(&mut self, spec: ToolSpec, handler: Box<dyn ToolHandler>) {
        self.handlers.insert(spec.name.clone(), handler);
        self.specs.push(spec);
    }

    /// Get all tool specs (for passing to the LLM).
    pub fn specs(&self) -> &[ToolSpec] {
        &self.specs
    }

    /// Whether the registry has any tools.
    pub fn is_empty(&self) -> bool {
        self.specs.is_empty()
    }

    /// Dispatch a tool call to the registered handler.
    ///
    /// Returns a `ToolResult` with `is_error = true` when the tool is not
    /// found or the handler returns an error, so the LLM always gets feedback.
    pub async fn dispatch(&self, name: &str, args: Value) -> ToolResult {
        let handler = match self.handlers.get(name) {
            Some(h) => h,
            None => {
                return ToolResult {
                    content: format!("Unknown tool: {name}"),
                    is_error: true,
                };
            }
        };

        match handler.execute(args).await {
            Ok(result) => result,
            Err(e) => ToolResult {
                content: e.to_string(),
                is_error: true,
            },
        }
    }
}

/// Validate that a path is within the workspace boundary.
///
/// Resolves the path relative to `cwd`, then verifies the canonical path
/// falls within `cwd`. Rejects `..` traversal and symlink escapes.
pub fn validate_path(path: &str, cwd: &Path) -> Result<PathBuf, ToolError> {
    let target = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        cwd.join(path)
    };

    // Use dunce::canonicalize on Windows to avoid \\?\ prefix issues.
    let resolved = dunce::canonicalize(&target).map_err(|e| {
        ToolError::Execution(format!(
            "failed to resolve path '{}': {e}",
            target.display()
        ))
    })?;

    let cwd_canonical = dunce::canonicalize(cwd).map_err(|e| {
        ToolError::Execution(format!(
            "failed to resolve workspace path '{}': {e}",
            cwd.display()
        ))
    })?;

    if !resolved.starts_with(&cwd_canonical) {
        return Err(ToolError::Execution(format!(
            "path '{}' is outside the workspace boundary",
            resolved.display()
        )));
    }

    Ok(resolved)
}

/// Check whether a file exceeds the maximum allowed size.
///
/// Returns `Some(ToolResult)` with a user-friendly error when the file is
/// too large, or `None` when the size is acceptable.
pub fn check_file_size(path: &Path) -> Option<ToolResult> {
    let size = match std::fs::metadata(path) {
        Ok(m) => m.len(),
        Err(_) => return None, // let the caller handle open errors
    };
    if size > MAX_FILE_SIZE {
        let size_mb = size / (1024 * 1024);
        Some(ToolResult {
            content: format!(
                "File '{}' is too large ({size_mb} MB). Maximum allowed size is {} MB.",
                path.display(),
                MAX_FILE_SIZE / (1024 * 1024)
            ),
            is_error: true,
        })
    } else {
        None
    }
}

/// Detect whether a file is binary by probing its first bytes for NUL.
///
/// Returns `Some(ToolResult)` with a user-friendly error when the file
/// appears to be binary, or `None` when it looks like text.
pub fn check_binary(path: &Path) -> Option<ToolResult> {
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return None, // let the caller handle open errors
    };
    let mut probe = vec![0u8; BINARY_PROBE_SIZE];
    let n = match std::io::Read::read(&mut file, &mut probe) {
        Ok(n) => n,
        Err(_) => return None,
    };
    if probe[..n].contains(&0) {
        Some(ToolResult {
            content: format!(
                "File '{}' appears to be a binary file and cannot be read as text.",
                path.display()
            ),
            is_error: true,
        })
    } else {
        None
    }
}
