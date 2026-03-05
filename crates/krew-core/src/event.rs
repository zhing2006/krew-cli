use std::collections::HashMap;
use std::sync::Arc;

use krew_llm::{ChatMessage, Usage};
use tokio::sync::Mutex;

/// User's decision on a tool approval request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ReviewDecision {
    /// Approve execution this time only.
    Approved,
    /// Approve and skip approval for the same tool+args this session.
    ApprovedForSession,
    /// Deny execution; agent loop returns an error result to the LLM.
    #[default]
    Denied,
    /// Abort the entire agent turn.
    Abort,
}

/// Session-scoped cache of tool approval decisions.
///
/// Cloning shares the same underlying cache (via `Arc`), so all agents
/// in a session see the same approvals.
#[derive(Clone, Default)]
pub struct ApprovalCache {
    inner: Arc<Mutex<HashMap<String, ReviewDecision>>>,
}

impl ApprovalCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a tool has a cached session approval.
    pub async fn is_approved(&self, tool_name: &str) -> bool {
        let cache = self.inner.lock().await;
        cache.get(tool_name) == Some(&ReviewDecision::ApprovedForSession)
    }

    /// Record a session-wide approval for a tool.
    pub async fn approve_for_session(&self, tool_name: String) {
        let mut cache = self.inner.lock().await;
        cache.insert(tool_name, ReviewDecision::ApprovedForSession);
    }
}

/// Events emitted by the agent loop for consumption by the TUI layer.
///
/// Note: `AgentEvent` cannot derive `Clone` because `ApprovalRequest`
/// contains a `oneshot::Sender` which is not cloneable.
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
    /// A server-side tool (e.g. web_search) has started executing.
    ServerToolStart { name: String },
    /// A server-side tool has completed with optional query/context.
    ServerToolDone { name: String, query: Option<String> },
    /// A tool call is starting execution.
    ToolCallStart { name: String, arguments: String },
    /// Incremental output from a streaming tool (e.g. shell).
    ToolCallOutput { text: String },
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
        /// Server-side tool uses collected across all rounds (for persistence).
        server_tool_uses: Vec<krew_llm::ServerToolUseInfo>,
    },
    /// An error occurred during the agent turn.
    Error {
        message: String,
        /// Any intermediate messages collected before the error occurred.
        intermediate_messages: Vec<ChatMessage>,
    },
    /// A tool requires user approval before execution.
    ///
    /// The agent loop blocks on the oneshot receiver until the TUI sends
    /// a `ReviewDecision` via the sender.
    ApprovalRequest {
        /// Tool name (e.g. "shell", "edit_file").
        tool_name: String,
        /// Raw JSON arguments string for display.
        arguments: String,
        /// Whether the "Approve for Session" option should be shown.
        ///
        /// Set to `false` for shell commands that cannot be reliably parsed
        /// (complex constructs), since session-wide approval would be unsafe.
        allow_session_approval: bool,
        /// Channel to send the user's decision back to the agent loop.
        respond: tokio::sync::oneshot::Sender<ReviewDecision>,
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
