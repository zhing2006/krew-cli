pub mod types;

pub mod anthropic;
pub mod common;
pub mod google;
pub mod list_models;
pub mod openai_chat;
pub mod openai_responses;
pub mod vertex_anthropic;

pub use anthropic::AnthropicClient;
pub use google::GoogleClient;
pub use krew_config::OtherAgentRole;
pub use list_models::{ListModelsConfig, ModelInfo, fallback_models, list_models};
pub use openai_chat::OpenAiChatClient;
pub use openai_responses::OpenAiResponsesClient;
pub use types::*;
pub use vertex_anthropic::VertexAnthropicClient;

use chrono::{DateTime, Utc};
use futures::Stream;
use krew_config::{RetryConfig, SamplingConfig, ThinkingEffort};
use std::pin::Pin;

/// Common configuration shared by all LLM client constructors.
///
/// Groups the parameters that every provider needs, avoiding long argument
/// lists in individual `new()` methods.
pub struct LlmClientConfig {
    /// Agent name for multi-agent message role attribution.
    pub agent_name: String,
    /// LLM model identifier.
    pub model: String,
    /// Resolved API key value.
    pub api_key: String,
    /// Optional base URL override for the provider API.
    pub base_url: Option<String>,
    /// How to present other agents' messages in conversation history.
    pub other_agent_role: OtherAgentRole,
    /// Retry configuration for API requests.
    pub retry_config: RetryConfig,
    /// Whether thinking/reasoning is enabled for this agent.
    pub enable_thinking: bool,
    /// Thinking effort level (only used when `enable_thinking` is true).
    pub thinking_effort: Option<ThinkingEffort>,
    /// Whether to inject the provider's native web search tool.
    pub enable_web_search: bool,
    /// Extra HTTP headers to include in chat/inference requests.
    pub extra_headers: Vec<(String, String)>,
}

/// Errors that can occur during LLM API interactions.
#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("API error: {0}")]
    Api(String),

    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("authentication error: {0}")]
    Auth(String),

    #[error("stream error: {0}")]
    Stream(String),
}

/// Unified trait for LLM provider clients.
///
/// All providers (OpenAI, Anthropic, Google, OpenAI-Compatible) implement
/// this trait to provide streaming chat completions.
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    /// Send messages to the LLM and receive a stream of events.
    ///
    /// The optional `on_retry` callback is invoked before each retry sleep,
    /// allowing the caller (e.g. TUI) to display retry status.
    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        sampling: &SamplingConfig,
        on_retry: Option<&(dyn Fn(common::RetryInfo) + Send + Sync)>,
    ) -> Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>, LlmError>;
}

/// Image content attached to a message (e.g. from read_file on an image).
#[derive(Debug, Clone)]
pub struct ImageContent {
    /// Raw image bytes.
    pub data: Vec<u8>,
    /// MIME type (e.g. "image/png", "image/jpeg").
    pub media_type: String,
    /// Original filename (e.g. "screenshot.png").
    pub filename: Option<String>,
}

/// A single extended-thinking block produced by a provider that supports
/// reasoning with signed payloads (currently Anthropic Messages API).
///
/// `Thinking` carries the visible reasoning text plus the opaque `signature`
/// the server needs to verify the block on the next request. `Redacted`
/// represents a block whose plaintext was withheld for safety reasons; only
/// the opaque `data` blob must be echoed back unchanged.
#[derive(Debug, Clone, PartialEq)]
pub enum ThinkingBlock {
    Thinking { text: String, signature: String },
    Redacted { data: String },
}

/// Unified message format used when communicating with LLM providers.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// Message role (system, user, assistant, or tool).
    pub role: ChatRole,
    /// Message text content.
    pub content: String,
    /// Optional agent name for multi-agent context.
    pub name: Option<String>,
    /// Tool calls made by the assistant (only for Assistant messages).
    pub tool_calls: Option<Vec<ToolCallInfo>>,
    /// Tool call ID this message is responding to (only for Tool messages).
    pub tool_call_id: Option<String>,
    /// Server-side tool uses (e.g. web_search) recorded for display/persistence.
    /// These are provider-executed tools, not dispatched by our tool system.
    pub server_tool_uses: Vec<ServerToolUseInfo>,
    /// Target addressee for user messages ("all", agent name, or None).
    pub addressee: Option<String>,
    /// Whisper targets: when set, only these agents (and the sender) can see the message.
    pub whisper_targets: Option<Vec<String>>,
    /// Timestamp when this message was created.
    pub created_at: DateTime<Utc>,
    /// Per-message token usage (assistant messages only).
    pub usage: Option<Usage>,
    /// Image data attached to this message (not persisted to session files).
    pub images: Vec<ImageContent>,
    /// Extended-thinking blocks emitted by the assistant on this turn.
    ///
    /// Always empty for non-assistant messages. Currently only Anthropic /
    /// Vertex Anthropic populate this; other providers ignore the field both
    /// when receiving streams and when serializing assistant history.
    pub thinking_blocks: Vec<ThinkingBlock>,
}

/// Information about a server-side tool use (e.g. web_search, google_search).
#[derive(Debug, Clone)]
pub struct ServerToolUseInfo {
    /// Tool name (e.g. "web_search").
    pub name: String,
    /// Optional query or context (e.g. search query string).
    pub query: Option<String>,
}

/// Information about a tool call made by the assistant.
#[derive(Debug, Clone)]
pub struct ToolCallInfo {
    /// Unique identifier for this tool call.
    pub id: String,
    /// Name of the tool being called.
    pub name: String,
    /// JSON-encoded arguments string.
    pub arguments: String,
    /// Opaque thought signature from Google thinking mode (must be echoed back).
    pub thought_signature: Option<String>,
}

impl ChatMessage {
    /// Create a simple text message (no tool calls).
    pub fn text(role: ChatRole, content: impl Into<String>, name: Option<String>) -> Self {
        Self {
            role,
            content: content.into(),
            name,
            tool_calls: None,
            tool_call_id: None,
            server_tool_uses: Vec::new(),
            addressee: None,
            whisper_targets: None,
            created_at: Utc::now(),
            usage: None,
            images: Vec::new(),
            thinking_blocks: Vec::new(),
        }
    }

    /// Create a user message with addressee information for persistence.
    pub fn user_with_addressee(content: impl Into<String>, addressee: Option<String>) -> Self {
        Self {
            role: ChatRole::User,
            content: content.into(),
            name: None,
            tool_calls: None,
            tool_call_id: None,
            server_tool_uses: Vec::new(),
            addressee,
            whisper_targets: None,
            created_at: Utc::now(),
            usage: None,
            images: Vec::new(),
            thinking_blocks: Vec::new(),
        }
    }

    /// Set whisper targets on this message (builder pattern).
    pub fn with_whisper_targets(mut self, targets: Option<Vec<String>>) -> Self {
        self.whisper_targets = targets;
        self
    }
}

/// Role of a message in the LLM conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thinking_block_variants_are_distinguishable() {
        let signed = ThinkingBlock::Thinking {
            text: "let me think".to_string(),
            signature: "sig".to_string(),
        };
        let redacted = ThinkingBlock::Redacted {
            data: "opaque".to_string(),
        };

        match signed {
            ThinkingBlock::Thinking { text, signature } => {
                assert_eq!(text, "let me think");
                assert_eq!(signature, "sig");
            }
            ThinkingBlock::Redacted { .. } => panic!("expected Thinking variant"),
        }

        match redacted {
            ThinkingBlock::Redacted { data } => assert_eq!(data, "opaque"),
            ThinkingBlock::Thinking { .. } => panic!("expected Redacted variant"),
        }
    }

    #[test]
    fn chat_message_text_defaults_thinking_blocks_empty() {
        let msg = ChatMessage::text(ChatRole::User, "hello", None);
        assert!(msg.thinking_blocks.is_empty());
    }

    #[test]
    fn chat_message_user_with_addressee_defaults_thinking_blocks_empty() {
        let msg = ChatMessage::user_with_addressee("hi", Some("agent1".into()));
        assert!(msg.thinking_blocks.is_empty());
    }
}
