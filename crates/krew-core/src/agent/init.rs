use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use krew_config::{ApiType, Config, ProviderType};
use krew_llm::{
    AnthropicClient, GoogleClient, LlmClient, LlmClientConfig, OpenAiChatClient,
    OpenAiResponsesClient, VertexAnthropicClient,
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

        let vertex_anthropic_location = if provider_config.provider_type
            == ProviderType::VertexAnthropic
        {
            let Some(project) = provider_config
                .vertex_project
                .as_deref()
                .filter(|value| !value.is_empty())
            else {
                warnings.push(format!(
                    "Agent '{}': vertex_project missing for vertex-anthropic provider '{}', skipped",
                    agent_config.name, agent_config.provider
                ));
                continue;
            };
            let Some(location) = provider_config
                .vertex_location
                .as_deref()
                .filter(|value| !value.is_empty())
            else {
                warnings.push(format!(
                    "Agent '{}': vertex_location missing for vertex-anthropic provider '{}', skipped",
                    agent_config.name, agent_config.provider
                ));
                continue;
            };
            Some((project.to_string(), location.to_string()))
        } else {
            None
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
            reasoning_mode: agent_config.reasoning_mode,
            reasoning_context: agent_config.reasoning_context,
            enable_web_search: agent_config.enable_web_search,
            extra_headers,
        };

        // Create LLM client based on provider type.
        let client: Arc<dyn LlmClient> = match provider_config.provider_type {
            ProviderType::OpenAI => {
                let api_type = agent_config
                    .api_type
                    .unwrap_or_else(|| provider_config.default_api_type());
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
            ProviderType::VertexAnthropic => {
                let (project, location) = vertex_anthropic_location
                    .expect("vertex-anthropic project and location validated");
                Arc::new(VertexAnthropicClient::new(client_config, project, location))
            }
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
            allow_rules: config.allow_rules.clone(),
            deny_rules: config.deny_rules.clone(),
            ask_rules: config.ask_rules.clone(),
            cwd: cwd
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
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

        let perms = sub_agent::run_agent_tool::PermissionConfig {
            approval_mode: runtime.approval_mode,
            approval_cache: runtime.approval_cache.clone(),
            allow_rules: runtime.allow_rules.clone(),
            deny_rules: runtime.deny_rules.clone(),
            ask_rules: runtime.ask_rules.clone(),
            cwd: runtime.cwd.clone(),
        };
        let tool = sub_agent::RunAgentTool::new(
            sub_agent_defs.clone(),
            Arc::clone(&runtime.client),
            runtime.config.sampling.clone().unwrap_or_default(),
            perms,
        );

        let spec = tool.spec();
        if let Some(registry) = Arc::get_mut(&mut runtime.tools) {
            registry.register(spec, Box::new(tool));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use krew_config::{AgentConfig, ProviderConfig};

    fn vertex_anthropic_config(project: Option<&str>, location: Option<&str>) -> Config {
        let mut config = Config::default();
        config.agents.push(AgentConfig {
            name: "claude".into(),
            display_name: "Claude".into(),
            provider: "vertex-anthropic".into(),
            model: "claude-opus-4-7".into(),
            api_type: None,
            color: "blue".into(),
            system_prompt: None,
            tools: false,
            enable_web_search: true,
            sampling: None,
            enable_thinking: false,
            thinking_effort: None,
            reasoning_mode: None,
            reasoning_context: None,
        });
        config.providers.insert(
            "vertex-anthropic".into(),
            ProviderConfig {
                provider_type: ProviderType::VertexAnthropic,
                api_key: Some("ya29.token".into()),
                api_key_env: None,
                base_url: Some("https://litellm.example.com/vertex_ai".into()),
                vertex_project: project.map(str::to_string),
                vertex_location: location.map(str::to_string),
                extra_headers: None,
            },
        );
        config
    }

    #[test]
    fn init_agents_creates_vertex_anthropic_agent() {
        let config = vertex_anthropic_config(Some("proj"), Some("global"));
        let result = init_agents(&config, None);
        assert!(result.warnings.is_empty());
        assert!(result.agents.contains_key("claude"));
    }

    #[test]
    fn init_agents_skips_vertex_anthropic_missing_project() {
        let config = vertex_anthropic_config(None, Some("global"));
        let result = init_agents(&config, None);
        assert!(!result.agents.contains_key("claude"));
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning.contains("vertex_project missing"))
        );
    }

    #[test]
    fn init_agents_skips_vertex_anthropic_missing_location() {
        let config = vertex_anthropic_config(Some("proj"), None);
        let result = init_agents(&config, None);
        assert!(!result.agents.contains_key("claude"));
        assert!(
            result
                .warnings
                .iter()
                .any(|warning| warning.contains("vertex_location missing"))
        );
    }
}
