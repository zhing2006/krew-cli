mod agent_loop;
mod approval;
mod init;
mod prepare;
mod prune;

use std::sync::Arc;

use krew_config::{AgentConfig, OtherAgentRole};
use krew_llm::{ChatMessage, ChatRole, ToolDefinition};
use krew_tools::ToolRegistry;
use tokio::sync::mpsc;

use crate::event::{AgentEvent, ApprovalCache, SharedApprovalMode};

pub use init::{InitAgentsResult, init_agents, register_sub_agents};

pub(crate) use agent_loop::{AgentLoopContext, run_agent_loop};
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
    /// Tool approval policy for this session (shared handle; runtime mode
    /// cycling is observed by in-flight loops at their next approval check).
    pub approval_mode: SharedApprovalMode,
    /// Session-scoped approval cache (persists across agent turns).
    pub approval_cache: ApprovalCache,
    /// Permission rules that auto-approve matching tool calls.
    pub allow_rules: Vec<krew_config::PermissionRule>,
    /// Permission rules that auto-deny matching tool calls.
    pub deny_rules: Vec<krew_config::PermissionRule>,
    /// Permission rules that force approval even in FullAuto mode.
    pub ask_rules: Vec<krew_config::PermissionRule>,
    /// Working directory for path normalization in permission rules.
    pub cwd: String,
    /// Pre-built skill catalog XML for system prompt injection.
    pub skill_catalog: Option<String>,
    /// Pre-built Sub-Agent catalog XML for system prompt injection.
    pub sub_agent_catalog: Option<String>,
    /// Language for agent responses (injected into system prompt when set).
    pub language: Option<String>,
}

/// Information about a peer agent, used for AI-to-AI prompt injection.
pub struct PeerAgent {
    pub name: String,
    pub display_name: String,
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
        peer_agents: Option<&[PeerAgent]>,
        whisper_targets: Option<Vec<String>>,
        exclude_tools: Option<&[&str]>,
    ) -> mpsc::UnboundedReceiver<AgentEvent> {
        let (tx, rx) = mpsc::unbounded_channel();

        // Send ResponseStart immediately so TUI can render the header.
        let _ = tx.send(AgentEvent::ResponseStart {
            agent_name: self.config.name.clone(),
            display_name: self.config.display_name.clone(),
            color: self.config.color.clone(),
        });

        // Build agent identity + optional custom system prompt.
        // Hour-aligned timestamp: stays stable within an hour, matching typical
        // provider prompt-cache TTLs (Anthropic 5min default / 1h beta, OpenAI
        // ~5-10min, Gemini implicit cache). Day-only precision was over-conservative
        // (cache never lives that long) and stripped useful time-of-day context.
        let now = chrono::Local::now()
            .format("%Y-%m-%d %H:00 (%A)")
            .to_string();
        let identity = build_identity_prompt(
            &self.config.display_name,
            &self.config.model,
            &self.config.name,
            &now,
            self.language.as_deref(),
            peer_agents,
            whisper_targets.as_deref(),
        );

        let agent_prompt = match &self.config.system_prompt {
            Some(prompt) if !prompt.is_empty() => format!("{identity}\n\n{prompt}"),
            _ => identity,
        };
        // Only inject memory write instructions when the agent actually has
        // file tools available (config.tools=true AND registry is non-empty).
        let has_file_tools = self.config.tools && !self.tools.specs().is_empty();
        let memory_prompt =
            crate::memory::load_memory_prompt(&self.config.name, &self.cwd, has_file_tools);
        let system_prompt = build_system_prompt(
            project_instructions,
            self.skill_catalog.as_deref(),
            self.sub_agent_catalog.as_deref(),
            memory_prompt.as_deref(),
            Some(&agent_prompt),
        );
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
        let approval_mode = self.approval_mode.clone();
        let approval_cache = self.approval_cache.clone();
        let allow_rules = self.allow_rules.clone();
        let deny_rules = self.deny_rules.clone();
        let ask_rules = self.ask_rules.clone();
        let cwd = self.cwd.clone();
        let exclude: Vec<String> = exclude_tools
            .unwrap_or(&[])
            .iter()
            .map(|s| s.to_string())
            .collect();

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

            // Convert ToolSpec -> ToolDefinition for the LLM API,
            // skipping any tools listed in exclude_tools.
            let tool_defs: Vec<ToolDefinition> = tools
                .specs()
                .iter()
                .filter(|spec| !exclude.contains(&spec.name))
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
                allow_rules: &allow_rules,
                deny_rules: &deny_rules,
                ask_rules: &ask_rules,
                cwd: &cwd,
                whisper_targets,
            };
            run_agent_loop(&ctx, &mut full_messages).await;
        });

        rx
    }
}

/// Build the final system prompt by merging project instructions, skill
/// catalog, sub-agent catalog, memory prompt, and the agent's configured
/// system_prompt.
///
/// Assembly order:
/// 1. `<project-instructions>` (if present)
/// 2. Skill catalog XML (if present)
/// 3. Sub-Agent catalog XML (if present)
/// 4. Memory prompt (if present)
/// 5. Agent system prompt
pub fn build_system_prompt(
    project_instructions: Option<&str>,
    skill_catalog: Option<&str>,
    sub_agent_catalog: Option<&str>,
    memory_prompt: Option<&str>,
    agent_system_prompt: Option<&str>,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    if let Some(instructions) = project_instructions {
        parts.push(format!(
            "<project-instructions>\n{instructions}\n</project-instructions>"
        ));
    }

    if let Some(catalog) = skill_catalog.filter(|c| !c.is_empty()) {
        parts.push(catalog.to_string());
    }

    if let Some(catalog) = sub_agent_catalog.filter(|c| !c.is_empty()) {
        parts.push(catalog.to_string());
    }

    if let Some(mem) = memory_prompt.filter(|m| !m.is_empty()) {
        parts.push(mem.to_string());
    }

    if let Some(prompt) = agent_system_prompt.filter(|p| !p.is_empty()) {
        parts.push(prompt.to_string());
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

/// Build the language instruction string for system prompt injection.
///
/// Returns a newline-prefixed instruction when `language` is `Some`,
/// or an empty string when `None`.
pub fn build_language_instruction(language: Option<&str>) -> String {
    match language {
        Some(lang) => format!(
            "\nAlways respond in {lang}. Use {lang} for all explanations, comments, and communications with the user. Technical terms and code identifiers should remain in their original form."
        ),
        None => String::new(),
    }
}

/// Build the complete identity prompt for an agent.
///
/// Assembly order:
/// 1. Core identity (name, model, date/time, language instruction)
/// 2. Peer agent collaboration hints (if peers exist)
/// 3. Whisper privacy context (if in whisper mode)
pub fn build_identity_prompt(
    display_name: &str,
    model: &str,
    agent_name: &str,
    now: &str,
    language: Option<&str>,
    peer_agents: Option<&[PeerAgent]>,
    whisper_targets: Option<&[String]>,
) -> String {
    let other_agent_hint = "Their messages are prefixed with [agent_name] in the content. User messages are prefixed with [user].";
    let language_instruction = build_language_instruction(language);

    // 1. Core identity block.
    let identity = format!(
        "You are {display_name}, powered by the {model} model.\n\
         Your agent name in this conversation is \"{agent_name}\".\n\
         You are participating in a multi-agent conversation hosted by krew-cli.\n\
         krew-cli is a multi-AI-agent collaborative CLI tool where users chat with multiple LLMs simultaneously in one terminal.\n\
         To help the user modify krew configuration, run `krew config help` to get the full configuration manual.\n\
         Other agents in this conversation are DIFFERENT AI models, not you. \
         {other_agent_hint}\n\
         Respond as yourself — do not role-play or impersonate other agents.\n\
         Current date: {now}{language_instruction}",
    );

    // 2. Peer agent collaboration hint.
    let identity = if let Some(peers) = peer_agents.filter(|p| !p.is_empty()) {
        let peer_list: Vec<String> = peers
            .iter()
            .map(|p| format!("[{}] {}", p.name, p.display_name))
            .collect();
        format!(
            "{identity}\n\
             You can ask another agent to respond by writing @name (with spaces before and after, e.g. \" @opus \").\n\
             Only use @name when you need that agent to reply — do NOT use @ when merely mentioning an agent by name.\n\
             Other agents: {}.",
            peer_list.join(", ")
        )
    } else {
        identity
    };

    // 3. Whisper context layers.
    if let Some(targets) = whisper_targets {
        let other_members: Vec<&String> = targets
            .iter()
            .filter(|t| t.as_str() != agent_name)
            .collect();

        // Layer 1: Privacy context (always injected when whisper).
        let privacy = if other_members.is_empty() {
            "You are in a private whisper conversation with the user. Other agents cannot see this conversation.".to_string()
        } else {
            let member_list: Vec<String> = other_members.iter().map(|n| format!("@{n}")).collect();
            format!(
                "You are in a private whisper conversation with the user and {}. \
                 Agents outside this group cannot see the conversation content.",
                member_list.join(", ")
            )
        };

        // Layer 2: Scope — clarify that everything in this round is whisper-scoped.
        let scope = "Everything in this conversation round — your response, tool calls, \
                     and tool results — is part of this whisper and only visible to \
                     whisper group members.";

        // Layer 3: Confidentiality — prevent leaking whisper content in normal messages.
        let confidentiality = if other_members.is_empty() {
            "IMPORTANT: In subsequent non-whisper (normal) messages, you must NEVER reveal, \
             reference, quote, or summarize any content from whisper conversations. \
             Treat all whisper content as strictly confidential. However, if the user \
             starts another whisper with you, you may freely reference previous whisper \
             content with them."
                .to_string()
        } else {
            let member_list: Vec<String> = other_members.iter().map(|n| format!("@{n}")).collect();
            format!(
                "IMPORTANT: In subsequent non-whisper (normal) messages, you must NEVER reveal, \
                 reference, quote, or summarize any content from whisper conversations. \
                 Treat all whisper content as strictly confidential. However, if the same \
                 whisper group ({}) reconvenes, you may freely reference previous whisper \
                 content within that group.",
                member_list.join(", ")
            )
        };

        // Layer 4: @mention collaboration (only when A2A enabled and multi-member group).
        let a2a_hint = if !other_members.is_empty() && peer_agents.is_some_and(|p| !p.is_empty()) {
            format!(
                "\nIn this whisper group, you may only @mention group members: {}. \
                     Mentions of agents outside the group will be ignored.",
                other_members
                    .iter()
                    .map(|n| format!("@{n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        } else {
            String::new()
        };

        format!("{identity}\n{privacy}\n{scope}\n{confidentiality}{a2a_hint}")
    } else {
        identity
    }
}
