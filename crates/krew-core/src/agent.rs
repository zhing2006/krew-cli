use std::sync::Arc;

use futures::StreamExt;
use krew_config::AgentConfig;
use krew_llm::{ChatMessage, ChatRole, LlmClient, StreamEvent, ToolDefinition};
use krew_tools::Tool;
use tokio::sync::mpsc;

use crate::event::AgentEvent;

/// Runtime state for a single agent in a session.
pub struct AgentRuntime {
    /// Agent configuration from settings.
    pub config: AgentConfig,
    /// LLM client for this agent's provider.
    pub client: Arc<dyn LlmClient>,
    /// Tools available to this agent.
    pub tools: Vec<Box<dyn Tool>>,
    /// Whether the agent is currently generating a response.
    pub is_responding: bool,
    /// Whether this agent's provider uses the `name` field on messages.
    pub use_name_field: bool,
}

impl AgentRuntime {
    /// Start a streaming completion for this agent.
    ///
    /// Returns a channel receiver immediately. The HTTP request and stream
    /// consumption run in a spawned background task so the caller's event
    /// loop is never blocked.
    pub fn start_completion(
        &self,
        messages: Vec<ChatMessage>,
        project_instructions: Option<&str>,
    ) -> mpsc::UnboundedReceiver<AgentEvent> {
        let (tx, rx) = mpsc::unbounded_channel();

        // Send ResponseStart immediately so TUI can render the header.
        let _ = tx.send(AgentEvent::ResponseStart {
            agent_name: self.config.name.clone(),
            display_name: self.config.display_name.clone(),
            color: self.config.color.clone(),
        });

        // Build agent identity + optional custom system prompt.
        let other_agent_hint = if self.use_name_field {
            "Their messages carry a \"name\" field identifying the source agent."
        } else {
            "Their messages are prefixed with [agent_name] in the content."
        };
        let identity = format!(
            "You are {display_name}, powered by the {model} model.\n\
             Your agent name in this conversation is \"{name}\".\n\
             You are participating in a multi-agent conversation hosted by krew-cli.\n\
             Other agents in this conversation are DIFFERENT AI models, not you. \
             {other_agent_hint}\n\
             Respond as yourself — do not role-play or impersonate other agents.",
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
            full_messages.push(ChatMessage {
                role: ChatRole::System,
                content: prompt,
                name: None,
            });
        }
        full_messages.extend(messages);

        let sampling = self.config.sampling.clone().unwrap_or_default();
        let tools: Vec<ToolDefinition> = vec![]; // Phase 4: no tools
        let agent_name = self.config.name.clone();
        let client = Arc::clone(&self.client);

        // Spawn the HTTP request + stream consumption so the event loop
        // is free to redraw immediately.
        tokio::spawn(async move {
            match client.chat_stream(&full_messages, &tools, &sampling).await {
                Ok(stream) => {
                    consume_stream(stream, tx, &agent_name).await;
                }
                Err(e) => {
                    let _ = tx.send(AgentEvent::Error(e.to_string()));
                }
            }
        });

        rx
    }
}

/// Consume an LLM stream and forward events through the channel.
async fn consume_stream(
    mut stream: std::pin::Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>,
    tx: mpsc::UnboundedSender<AgentEvent>,
    agent_name: &str,
) {
    while let Some(event) = stream.next().await {
        let agent_event = match event {
            StreamEvent::TextDelta(text) => AgentEvent::TextDelta(text),
            StreamEvent::Done(usage) => {
                let _ = tx.send(AgentEvent::Done(usage));
                return;
            }
            StreamEvent::Error(msg) => {
                let _ = tx.send(AgentEvent::Error(msg));
                return;
            }
            StreamEvent::ToolCall { .. } => {
                // Phase 4: skip tool calls.
                tracing::debug!(agent = agent_name, "skipping tool call (not implemented)");
                continue;
            }
            StreamEvent::ThinkingDelta(text) => AgentEvent::ThinkingDelta(text),
        };

        if tx.send(agent_event).is_err() {
            // Receiver dropped — stop consuming.
            return;
        }
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
