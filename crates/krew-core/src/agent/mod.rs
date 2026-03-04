mod agent_loop;
mod approval;
mod init;
mod prepare;
mod prune;

use std::sync::Arc;

use krew_config::{AgentConfig, ApprovalMode, OtherAgentRole};
use krew_llm::{ChatMessage, ChatRole, ToolDefinition};
use krew_tools::ToolRegistry;
use tokio::sync::mpsc;

use crate::event::{AgentEvent, ApprovalCache};

pub use init::{InitAgentsResult, init_agents};

use agent_loop::{AgentLoopContext, run_agent_loop};
use prepare::prepare_messages_for_agent;

/// Default maximum number of tool-call loop rounds per agent turn.
const DEFAULT_MAX_TOOL_ROUNDS: u32 = 25;

/// Runtime state for a single agent in a session.
pub struct AgentRuntime {
    /// Agent configuration from settings.
    pub config: AgentConfig,
    /// LLM client for this agent's provider.
    pub client: Arc<dyn krew_llm::LlmClient>,
    /// Tools available to this agent.
    pub tools: Arc<ToolRegistry>,
    /// Whether the agent is currently generating a response.
    pub is_responding: bool,
    /// How to present other agents' messages in this agent's conversation.
    pub other_agent_role: OtherAgentRole,
    /// Tool approval policy for this session.
    pub approval_mode: ApprovalMode,
    /// Session-scoped approval cache (persists across agent turns).
    pub approval_cache: ApprovalCache,
    /// Shell commands that are auto-approved without user confirmation.
    pub shell_allow_commands: Vec<String>,
}

impl AgentRuntime {
    /// Start a streaming completion for this agent.
    ///
    /// Returns a channel receiver immediately. The HTTP request, stream
    /// consumption, and tool-call loop run in a spawned background task
    /// so the caller's event loop is never blocked.
    pub fn start_completion(
        &self,
        messages: Vec<ChatMessage>,
        project_instructions: Option<&str>,
        max_tool_rounds: Option<u32>,
    ) -> mpsc::UnboundedReceiver<AgentEvent> {
        let (tx, rx) = mpsc::unbounded_channel();

        // Send ResponseStart immediately so TUI can render the header.
        let _ = tx.send(AgentEvent::ResponseStart {
            agent_name: self.config.name.clone(),
            display_name: self.config.display_name.clone(),
            color: self.config.color.clone(),
        });

        // Build agent identity + optional custom system prompt.
        let other_agent_hint = "Their messages are prefixed with [agent_name] in the content.";
        let now = chrono::Local::now()
            .format("%Y-%m-%d %H:%M (%A)")
            .to_string();
        let identity = format!(
            "You are {display_name}, powered by the {model} model.\n\
             Your agent name in this conversation is \"{name}\".\n\
             You are participating in a multi-agent conversation hosted by krew-cli.\n\
             Other agents in this conversation are DIFFERENT AI models, not you. \
             {other_agent_hint}\n\
             Respond as yourself — do not role-play or impersonate other agents.\n\
             Current date/time: {now}",
            display_name = self.config.display_name,
            model = self.config.model,
            name = self.config.name,
        );
        let agent_prompt = match &self.config.system_prompt {
            Some(prompt) if !prompt.is_empty() => format!("{identity}\n\n{prompt}"),
            _ => identity,
        };
        let system_prompt = build_system_prompt(project_instructions, Some(&agent_prompt));
        let mut full_messages = Vec::with_capacity(messages.len() + 1);
        if let Some(prompt) = system_prompt {
            full_messages.push(ChatMessage::text(ChatRole::System, prompt, None));
        }
        full_messages.extend(prepare_messages_for_agent(messages, &self.config.name));

        let sampling = self.config.sampling.clone().unwrap_or_default();
        let agent_name = self.config.name.clone();
        let client = Arc::clone(&self.client);
        let tools = Arc::clone(&self.tools);
        let max_rounds = max_tool_rounds.unwrap_or(DEFAULT_MAX_TOOL_ROUNDS);
        let approval_mode = self.approval_mode;
        let approval_cache = self.approval_cache.clone();
        let shell_allow_commands = self.shell_allow_commands.clone();

        // Spawn the HTTP request + stream consumption + tool loop so the
        // event loop is free to redraw immediately.
        tokio::spawn(async move {
            // Build retry callback that forwards retry info to the TUI.
            let tx_retry = tx.clone();
            let on_retry = move |info: krew_llm::common::RetryInfo| {
                let _ = tx_retry.send(AgentEvent::Retrying {
                    attempt: info.attempt,
                    max_attempts: info.max_attempts,
                    reason: info.reason.clone(),
                    delay_secs: info.delay_secs,
                });
            };

            // Convert ToolSpec -> ToolDefinition for the LLM API.
            let tool_defs: Vec<ToolDefinition> = tools
                .specs()
                .iter()
                .map(|spec| ToolDefinition {
                    name: spec.name.clone(),
                    description: spec.description.clone(),
                    parameters: spec.parameters.clone(),
                })
                .collect();

            let ctx = AgentLoopContext {
                client: &client,
                tools: &tools,
                tool_defs: &tool_defs,
                sampling: &sampling,
                on_retry: &on_retry,
                tx: &tx,
                agent_name: &agent_name,
                max_rounds,
                approval_mode,
                approval_cache: &approval_cache,
                shell_allow_commands: &shell_allow_commands,
            };
            run_agent_loop(&ctx, &mut full_messages).await;
        });

        rx
    }
}

/// Build the final system prompt by merging project instructions with the
/// agent's configured system_prompt.
///
/// When project instructions are present, they are wrapped in
/// `<project-instructions>` tags and prepended before the agent's own prompt.
pub fn build_system_prompt(
    project_instructions: Option<&str>,
    agent_system_prompt: Option<&str>,
) -> Option<String> {
    match (project_instructions, agent_system_prompt) {
        (Some(instructions), Some(prompt)) if !prompt.is_empty() => Some(format!(
            "<project-instructions>\n{instructions}\n</project-instructions>\n\n{prompt}"
        )),
        (Some(instructions), _) => Some(format!(
            "<project-instructions>\n{instructions}\n</project-instructions>"
        )),
        (None, Some(prompt)) if !prompt.is_empty() => Some(prompt.to_string()),
        _ => None,
    }
}
