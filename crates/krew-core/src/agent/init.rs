use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use krew_config::{ApiType, Config, ProviderType};
use krew_llm::{
    AnthropicClient, GoogleClient, LlmClient, LlmClientConfig, OpenAiChatClient,
    OpenAiResponsesClient,
};
use krew_tools::ToolRegistry;
use krew_tools::builtin::SkillInfo;

use crate::event::ApprovalCache;
use crate::skill::{self, SkillRecord};
use crate::sub_agent::{self, SubAgentDef};

use super::AgentRuntime;

/// Result of agent initialization.
pub struct InitAgentsResult {
    /// Agent runtimes keyed by agent name.
    pub agents: HashMap<String, AgentRuntime>,
    /// Warning messages for agents that could not be initialized.
    pub warnings: Vec<String>,
    /// Discovered Agent Skills.
    pub skills: Vec<SkillRecord>,
    /// Discovered Sub-Agent definitions (empty when `sub_agent_enabled` is false).
    pub sub_agent_defs: Vec<SubAgentDef>,
}

/// Build `AgentRuntime` instances from configuration.
///
/// Iterates over `config.agents`, resolves API keys (from config value or
/// environment variable), creates provider-specific `LlmClient` instances,
/// and constructs `AgentRuntime` for each valid agent.
///
/// When `cwd` is provided and `agent_config.tools` is true, readonly built-in
/// tools are registered for the agent.
///
/// Agents that fail initialization (missing API key, unknown provider) are
/// skipped with a warning message returned in `InitAgentsResult::warnings`.
pub fn init_agents(config: &Config, cwd: Option<PathBuf>) -> InitAgentsResult {
    let mut agents = HashMap::new();
    let mut warnings = Vec::new();
    let shared_approval_cache = ApprovalCache::new();

    // Discover Agent Skills if enabled.
    let skills = if config.skills.enabled {
        if let Some(ref cwd_path) = cwd {
            let extra: Vec<PathBuf> = config
                .skills
                .extra_paths
                .iter()
                .map(PathBuf::from)
                .collect();
            skill::discover_skills(cwd_path, &extra)
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // Discover Sub-Agent definitions if enabled.
    let sub_agent_defs = if config.settings.sub_agent_enabled {
        if let Some(ref cwd_path) = cwd {
            sub_agent::discover_sub_agents(cwd_path)
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // Build skill catalog for system prompt injection.
    let skill_catalog = if skills.is_empty() {
        None
    } else {
        Some(skill::build_skill_catalog(&skills))
    };

    // Build sub-agent catalog for system prompt injection.
    let sub_agent_catalog = if sub_agent_defs.is_empty() {
        None
    } else {
        Some(sub_agent::build_sub_agent_catalog(&sub_agent_defs))
    };

    // Build SkillInfo map for the activate_skill tool.
    let skill_infos: HashMap<String, SkillInfo> = skills
        .iter()
        .map(|s| {
            (
                s.name.clone(),
                SkillInfo {
                    location: s.location.clone(),
                    base_dir: s.base_dir.clone(),
                },
            )
        })
        .collect();

    for agent_config in &config.agents {
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
        let extra_headers: Vec<(String, String)> = provider_config
            .extra_headers
            .as_ref()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        let client_config = LlmClientConfig {
            agent_name: agent_config.name.clone(),
            model: agent_config.model.clone(),
            api_key,
            base_url: provider_config.base_url.clone(),
            other_agent_role: config.settings.other_agent_role,
            retry_config: config.settings.retry.clone(),
            enable_thinking: agent_config.enable_thinking,
            thinking_effort: agent_config.thinking_effort,
            enable_web_search: agent_config.enable_web_search,
            extra_headers,
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

        // Create tool registry for this agent.
        let tools = if agent_config.tools {
            if let Some(ref cwd) = cwd {
                Arc::new(krew_tools::builtin::create_full_registry(
                    cwd.clone(),
                    config.settings.restrict_workspace,
                    skill_infos.clone(),
                ))
            } else {
                Arc::new(ToolRegistry::empty())
            }
        } else {
            Arc::new(ToolRegistry::empty())
        };

        let runtime = AgentRuntime {
            config: agent_config.clone(),
            client,
            tools,
            is_responding: false,
            other_agent_role: config.settings.other_agent_role,
            approval_mode: config.settings.approval_mode,
            approval_cache: shared_approval_cache.clone(),
            shell_allow_commands: config.settings.shell_allow_commands.clone(),
            fetch_allow_domains: config.settings.fetch_allow_domains.clone(),
            skill_catalog: skill_catalog.clone(),
            sub_agent_catalog: sub_agent_catalog.clone(),
            language: config.settings.language.clone(),
        };

        agents.insert(agent_config.name.clone(), runtime);
    }

    InitAgentsResult {
        agents,
        warnings,
        skills,
        sub_agent_defs,
    }
}

/// Register the `run_agent` tool into every agent that has tools enabled.
///
/// Must be called after MCP initialization (if any) so the tool registry
/// is in its final mutable state.
pub fn register_sub_agents(
    agents: &mut HashMap<String, AgentRuntime>,
    sub_agent_defs: Vec<SubAgentDef>,
) {
    if sub_agent_defs.is_empty() {
        return;
    }

    for runtime in agents.values_mut() {
        // Only register for agents that have tools enabled.
        if runtime.tools.is_empty() {
            continue;
        }

        let tool = sub_agent::RunAgentTool::new(
            sub_agent_defs.clone(),
            Arc::clone(&runtime.client),
            runtime.approval_mode,
            runtime.approval_cache.clone(),
            runtime.config.sampling.clone().unwrap_or_default(),
            runtime.shell_allow_commands.clone(),
            runtime.fetch_allow_domains.clone(),
        );

        let spec = tool.spec();
        if let Some(registry) = Arc::get_mut(&mut runtime.tools) {
            registry.register(spec, Box::new(tool));
        }
    }
}
