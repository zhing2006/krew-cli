use std::collections::HashMap;
use std::sync::Arc;

use futures::StreamExt;
use krew_config::{AgentConfig, ApiType, Config, OtherAgentRole, ProviderType};
use krew_llm::{
    AnthropicClient, ChatMessage, ChatRole, GoogleClient, LlmClient, LlmClientConfig,
    OpenAiChatClient, OpenAiResponsesClient, StreamEvent, ToolDefinition,
};
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
    /// How to present other agents' messages in this agent's conversation.
    pub other_agent_role: OtherAgentRole,
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
        let other_agent_hint = "Their messages are prefixed with [agent_name] in the content.";
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

            match client
                .chat_stream(&full_messages, &tools, &sampling, Some(&on_retry))
                .await
            {
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

/// Result of agent initialization.
pub struct InitAgentsResult {
    /// Agent runtimes keyed by agent name.
    pub agents: HashMap<String, AgentRuntime>,
    /// Warning messages for agents that could not be initialized.
    pub warnings: Vec<String>,
}

/// Build `AgentRuntime` instances from configuration.
///
/// Iterates over `config.agents`, resolves API keys (from config value or
/// environment variable), creates provider-specific `LlmClient` instances,
/// and constructs `AgentRuntime` for each valid agent.
///
/// Agents that fail initialization (missing API key, unknown provider) are
/// skipped with a warning message returned in `InitAgentsResult::warnings`.
pub fn init_agents(config: &Config) -> InitAgentsResult {
    let mut agents = HashMap::new();
    let mut warnings = Vec::new();

    for agent_config in &config.agents {
        if agent_config.provider == "builtin" {
            // Skip builtin echo agents — they don't need an LLM client.
            continue;
        }

        let provider_config = match config.providers.get(&agent_config.provider) {
            Some(p) => p,
            None => {
                warnings.push(format!(
                    "Agent '{}': provider '{}' not found, skipped",
                    agent_config.name, agent_config.provider
                ));
                continue;
            }
        };

        // Resolve API key: api_key takes precedence over api_key_env.
        let api_key = if let Some(key) = &provider_config.api_key {
            if key.is_empty() {
                warnings.push(format!(
                    "Agent '{}': api_key is empty, skipped",
                    agent_config.name
                ));
                continue;
            }
            key.clone()
        } else if let Some(env) = &provider_config.api_key_env {
            match std::env::var(env) {
                Ok(val) if !val.is_empty() => val,
                _ => {
                    warnings.push(format!(
                        "Agent '{}': env var '{}' not set or empty, skipped",
                        agent_config.name, env
                    ));
                    continue;
                }
            }
        } else {
            warnings.push(format!(
                "Agent '{}': no api_key or api_key_env configured, skipped",
                agent_config.name
            ));
            continue;
        };

        // Build shared client config.
        let client_config = LlmClientConfig {
            agent_name: agent_config.name.clone(),
            model: agent_config.model.clone(),
            api_key,
            base_url: provider_config.base_url.clone(),
            other_agent_role: config.settings.other_agent_role,
            retry_config: config.settings.retry.clone(),
            enable_thinking: agent_config.enable_thinking,
            thinking_effort: agent_config.thinking_effort,
        };

        // Create LLM client based on provider type.
        let client: Arc<dyn LlmClient> = match provider_config.provider_type {
            ProviderType::OpenAI => {
                let api_type = agent_config.api_type.unwrap_or(ApiType::Chat);
                match api_type {
                    ApiType::Chat => Arc::new(OpenAiChatClient::new(client_config)),
                    ApiType::Responses => Arc::new(OpenAiResponsesClient::new(client_config)),
                }
            }
            ProviderType::Anthropic => Arc::new(AnthropicClient::new(client_config)),
            ProviderType::Google => Arc::new(GoogleClient::new(
                client_config,
                provider_config.vertex_project.as_deref(),
                provider_config.vertex_location.as_deref(),
            )),
        };

        let runtime = AgentRuntime {
            config: agent_config.clone(),
            client,
            tools: Vec::new(),
            is_responding: false,
            other_agent_role: config.settings.other_agent_role,
        };

        agents.insert(agent_config.name.clone(), runtime);
    }

    InitAgentsResult { agents, warnings }
}
