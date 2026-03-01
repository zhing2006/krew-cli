pub mod types;

mod anthropic;
mod google;
pub mod openai_chat;
mod openai_responses;

pub use types::*;

use futures::Stream;
use krew_config::SamplingConfig;
use std::pin::Pin;

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
    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        sampling: &SamplingConfig,
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
}

/// Role of a message in the LLM conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}
