use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use krew_config::ApprovalMode;
use krew_llm::{ChatMessage, Usage};
use tokio::sync::Mutex;

/// Approval mode shared between the app, in-flight agent loops, and
/// sub-agent launches.
///
/// Cloning shares the same underlying value (via `Arc`), so runtime cycling
/// (Shift+Tab in the TUI) is observed by running loops at their next
/// tool-approval check without restarting completions.
#[derive(Debug, Clone)]
pub struct SharedApprovalMode(Arc<AtomicU8>);

impl SharedApprovalMode {
    pub fn new(mode: ApprovalMode) -> Self {
        Self(Arc::new(AtomicU8::new(Self::encode(mode))))
    }

    /// Current mode.
    pub fn get(&self) -> ApprovalMode {
        Self::decode(self.0.load(Ordering::Relaxed))
    }

    /// Replace the mode; visible to every clone of this handle.
    pub fn set(&self, mode: ApprovalMode) {
        self.0.store(Self::encode(mode), Ordering::Relaxed);
    }

    fn encode(mode: ApprovalMode) -> u8 {
        match mode {
            ApprovalMode::Suggest => 0,
            ApprovalMode::AutoEdit => 1,
            ApprovalMode::FullAuto => 2,
        }
    }

    fn decode(value: u8) -> ApprovalMode {
        match value {
            1 => ApprovalMode::AutoEdit,
            2 => ApprovalMode::FullAuto,
            _ => ApprovalMode::Suggest,
        }
    }
}

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

    /// Check if a cache key has a session approval.
    ///
    /// Keys are constructed by `cache_session_approval()`:
    /// - shell: `"shell:<command_prefix>"`
    /// - fetch_url: `"fetch_url:<host>"`
    /// - other tools: `"<tool_name>"`
    pub async fn is_approved(&self, key: &str) -> bool {
        let cache = self.inner.lock().await;
        cache.get(key) == Some(&ReviewDecision::ApprovedForSession)
    }

    /// Record a session-wide approval for a cache key.
    pub async fn approve_for_session(&self, key: String) {
        let mut cache = self.inner.lock().await;
        cache.insert(key, ReviewDecision::ApprovedForSession);
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
    ToolCallStart {
        name: String,
        /// Human-readable name for TUI (e.g. `mcp:server/tool`).
        display_name: String,
        arguments: String,
    },
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
        /// Thinking blocks aggregated from the final LLM turn, to be attached
        /// to the terminal assistant message for protocol-compliant replay.
        final_thinking_blocks: Vec<krew_llm::ThinkingBlock>,
        /// Raw, ordered content blocks from the final LLM turn, attached to the
        /// terminal assistant message so the next request can replay the exact
        /// thinking ↔ server_tool_use ↔ web_search_tool_result ↔ text order.
        final_raw_content_blocks: Vec<serde_json::Value>,
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
        /// Optional reason from an ask rule to display in the approval overlay.
        reason: Option<String>,
        /// Channel to send the user's decision back to the agent loop.
        respond: tokio::sync::oneshot::Sender<ReviewDecision>,
    },
    /// A tool call was denied by a permission rule (no user interaction).
    ToolDenied {
        /// Tool name that was denied.
        tool_name: String,
        /// Reason for denial (from the deny rule or built-in protection).
        reason: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_approval_mode_round_trips_all_modes() {
        for mode in [
            ApprovalMode::Suggest,
            ApprovalMode::AutoEdit,
            ApprovalMode::FullAuto,
        ] {
            assert_eq!(SharedApprovalMode::new(mode).get(), mode);
        }
    }

    #[test]
    fn shared_approval_mode_set_is_visible_to_clones() {
        let shared = SharedApprovalMode::new(ApprovalMode::Suggest);
        let clone = shared.clone();

        shared.set(ApprovalMode::FullAuto);
        assert_eq!(clone.get(), ApprovalMode::FullAuto);
    }
}
