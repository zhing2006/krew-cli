use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A message in the multi-agent conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Sender role (system, user, assistant, or tool).
    pub role: Role,
    /// Agent name when role is assistant.
    pub agent_name: Option<String>,
    /// Target addressee: "all" or a specific agent name.
    pub addressee: Option<String>,
    /// Message body content.
    pub content: MessageContent,
    /// Tool calls requested by the agent (if any).
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Results from tool executions (if any).
    pub tool_results: Option<Vec<ToolCallResult>>,
    /// Token usage for this message (assistant messages only).
    pub usage: Option<krew_llm::Usage>,
    /// Timestamp when the message was created.
    pub created_at: DateTime<Utc>,
}

/// Role of a message sender.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// Message content representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Plain text content.
    Text(String),
    /// Structured content blocks (Anthropic-style).
    Blocks(Vec<ContentBlock>),
}

/// A single content block within a structured message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlock {
    /// Block type identifier (e.g., "text", "tool_use").
    #[serde(rename = "type")]
    pub block_type: String,
    /// Text content of the block (if applicable).
    pub text: Option<String>,
}

/// A tool invocation requested by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for this tool call.
    pub id: String,
    /// Name of the tool to invoke.
    pub name: String,
    /// Arguments passed to the tool as JSON.
    pub arguments: serde_json::Value,
}

/// Result returned from a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    /// ID of the tool call this result corresponds to.
    pub tool_call_id: String,
    /// Output content from the tool.
    pub content: String,
    /// Whether the tool execution resulted in an error.
    pub is_error: bool,
}
