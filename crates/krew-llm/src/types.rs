use crate::ThinkingBlock;
use serde::{Deserialize, Serialize};

/// Events emitted during LLM streaming responses.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Incremental text content from the model.
    TextDelta(String),
    /// Model requests a tool invocation.
    ToolCall {
        id: String,
        name: String,
        arguments: String,
        /// Opaque signature for Google thinking mode (must be echoed back).
        thought_signature: Option<String>,
    },
    /// Incremental thinking/reasoning content (if supported).
    ThinkingDelta(String),
    /// A complete thinking block was finalized by the provider. Emitted once
    /// per block (after all `ThinkingDelta`s for that block), carrying the
    /// signature needed to replay the block on the next request.
    ThinkingBlockDone(ThinkingBlock),
    /// A server-side tool (e.g. web_search) has started executing.
    ServerToolStart { name: String },
    /// A server-side tool has completed, with optional query/context for display.
    ServerToolDone { name: String, query: Option<String> },
    /// Stream completed, carrying final token usage.
    Done(Usage),
    /// An error occurred during streaming.
    Error(String),
}

/// Token usage statistics for a single LLM request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Number of input (prompt) tokens.
    pub prompt_tokens: u32,
    /// Number of output (completion) tokens.
    pub completion_tokens: u32,
    /// Total tokens (prompt + completion).
    pub total_tokens: u32,
}

/// Tool definition exposed to LLM providers via the tools parameter.
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    /// Tool name (must match the registered tool).
    pub name: String,
    /// Human-readable description of what the tool does.
    pub description: String,
    /// JSON Schema describing the tool's input parameters.
    pub parameters: serde_json::Value,
}
