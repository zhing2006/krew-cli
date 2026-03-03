pub mod types;

pub mod anthropic;
pub mod common;
pub mod google;
pub mod openai_chat;
pub mod openai_responses;

pub use anthropic::AnthropicClient;
pub use google::GoogleClient;
pub use krew_config::OtherAgentRole;
pub use openai_chat::OpenAiChatClient;
pub use openai_responses::OpenAiResponsesClient;
pub use types::*;

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
        }
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
