//! Task engine input/output types.

use std::sync::Arc;

use krew_config::{ApprovalMode, PermissionRule, SamplingConfig};
use krew_llm::{ChatMessage, ToolDefinition, Usage};
use krew_tools::ToolRegistry;

use crate::event::ApprovalCache;

/// Input for a task engine execution.
///
/// All fields are explicitly provided by the caller — the task engine makes
/// no default assumptions about permissions, tool exposure, or prompts.
pub struct TaskRequest {
    /// User prompt sent to the LLM.
    pub prompt: String,
    /// Optional raw system prompt (no identity/memory/skill assembly).
    pub system_prompt: Option<String>,
    /// LLM client.
    pub client: Arc<dyn krew_llm::LlmClient>,
    /// Tool registry for dispatch execution.
    pub tools: Arc<ToolRegistry>,
    /// Tool definitions exposed to the LLM (may be a subset of the registry).
    pub tool_defs: Vec<ToolDefinition>,
    /// Sampling parameters (temperature, etc.).
    pub sampling: SamplingConfig,
    /// Maximum tool-call loop rounds.
    pub max_rounds: u32,
    /// Agent name used in message `name` fields.
    pub agent_name: String,
    /// Tool approval policy.
    pub approval_mode: ApprovalMode,
    /// Approval cache (can be shared with parent agent).
    pub approval_cache: ApprovalCache,
    /// Rules that auto-approve matching tool calls.
    pub allow_rules: Vec<PermissionRule>,
    /// Rules that auto-deny matching tool calls.
    pub deny_rules: Vec<PermissionRule>,
    /// Rules that force approval even in FullAuto mode.
    pub ask_rules: Vec<PermissionRule>,
    /// Working directory for path normalization in permission rules.
    pub cwd: String,
}

/// Output from a task engine execution.
pub struct TaskResult {
    /// Final text response from the LLM.
    pub final_text: String,
    /// Complete conversation history including intermediate tool-call messages
    /// and the final assistant text message.
    pub messages: Vec<ChatMessage>,
    /// Cumulative token usage.
    pub usage: Usage,
    /// Whether the task terminated with an error.
    pub is_error: bool,
}
