use krew_llm::{ChatMessage, Usage};

/// Events emitted by the agent loop for consumption by the TUI layer.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Agent response is starting (render header label).
    ResponseStart {
        agent_name: String,
        display_name: String,
        color: String,
    },
    /// Incremental thinking/reasoning content from the model.
    ThinkingDelta(String),
    /// Incremental text content from the model.
    TextDelta(String),
    /// A tool call is starting execution.
    ToolCallStart { name: String, arguments: String },
    /// A tool call has completed.
    ToolCallDone {
        name: String,
        result_summary: String,
    },
    /// Stream completed with final token usage and collected messages.
    Done {
        usage: Usage,
        /// Assistant+tool_calls and Tool result messages from all tool rounds.
        intermediate_messages: Vec<ChatMessage>,
        /// Final text-only response from the last LLM call.
        final_text: String,
    },
    /// An error occurred during the agent turn.
    Error {
        message: String,
        /// Any intermediate messages collected before the error occurred.
        intermediate_messages: Vec<ChatMessage>,
    },
    /// A retry attempt is about to happen (for TUI status display).
    Retrying {
        /// Current retry attempt (1-based).
        attempt: u32,
        /// Maximum attempts allowed for this error type.
        max_attempts: u32,
        /// Human-readable reason (e.g. "rate limit (429)").
        reason: String,
        /// Delay in seconds before the retry.
        delay_secs: f64,
    },
}
