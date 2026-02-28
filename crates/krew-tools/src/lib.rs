pub mod builtin;
pub mod mcp;

use serde_json::Value;

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

/// Trait for tools that agents can invoke during conversations.
///
/// Both built-in tools and MCP-discovered tools implement this trait.
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// Unique tool name exposed to LLM providers.
    fn name(&self) -> &str;
    /// Human-readable description of the tool's purpose.
    fn description(&self) -> &str;
    /// JSON Schema describing the tool's input parameters.
    fn parameters_schema(&self) -> Value;
    /// Whether this tool requires user approval before execution.
    fn requires_approval(&self) -> bool;
    /// Execute the tool with the given arguments.
    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError>;
}
