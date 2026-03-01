use krew_llm::Usage;

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
    /// Stream completed with final token usage.
    Done(Usage),
    /// An error occurred during the agent turn.
    Error(String),
}
