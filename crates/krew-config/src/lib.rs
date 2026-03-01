mod defaults;
pub mod instructions;

pub use defaults::*;
pub use instructions::load_project_instructions;

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Filename to look for when loading project-level instructions.
pub const PROJECT_INSTRUCTIONS_FILENAME: &str = "AGENTS.md";

/// Maximum size in bytes for a single project instructions file (100KB).
pub const PROJECT_INSTRUCTIONS_MAX_SIZE: usize = 102_400;

/// Default config file path relative to the working directory.
pub const CONFIG_FILENAME: &str = ".krew/settings.toml";

/// Configuration error type.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// File I/O error (e.g. config file not found).
    #[error("failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    /// TOML parse/deserialization error.
    #[error("config parse error: {0}")]
    Parse(#[from] toml::de::Error),
    /// Validation error (invalid references, duplicates, etc.).
    #[error("config validation error: {0}")]
    Validation(String),
}

/// Root configuration loaded from `.krew/settings.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Global settings.
    pub settings: Settings,
    /// List of configured agents.
    pub agents: Vec<AgentConfig>,
    /// Provider SDK configurations keyed by provider name.
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    /// MCP server definitions.
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
}

/// Default input history limit.
pub const DEFAULT_INPUT_HISTORY_LIMIT: usize = 1000;

/// Global settings controlling session behavior.
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    /// Tool approval policy: suggest, auto-edit, or full-auto.
    pub approval_mode: ApprovalMode,
    /// Agent reply order for `@all` broadcasts.
    pub reply_order: Vec<String>,
    /// Token threshold for auto-compact (0 or None = disabled).
    pub auto_compact_threshold: Option<u32>,
    /// Maximum number of input history entries to keep.
    #[serde(default = "default_input_history_limit")]
    pub input_history_limit: usize,
    /// Enable timing-based paste burst detection as a fallback when the
    /// terminal does not support bracketed paste (e.g. Windows).
    #[serde(default = "default_true")]
    pub paste_burst_detection: bool,
    /// Number of tokio worker threads (defaults to 4).
    #[serde(default = "default_worker_threads")]
    pub worker_threads: usize,
}

fn default_input_history_limit() -> usize {
    DEFAULT_INPUT_HISTORY_LIMIT
}

fn default_true() -> bool {
    true
}

/// Default number of tokio worker threads.
pub const DEFAULT_WORKER_THREADS: usize = 4;

fn default_worker_threads() -> usize {
    DEFAULT_WORKER_THREADS
}

/// Configuration for a single AI agent.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    /// Unique identifier used for `@` addressing.
    pub name: String,
    /// Human-readable display name shown in output.
    pub display_name: String,
    /// Provider name referencing the `[providers]` table.
    pub provider: String,
    /// LLM model identifier.
    pub model: String,
    /// OpenAI-specific: which API to use (Responses or Chat).
    pub api_type: Option<ApiType>,
    /// Terminal color for this agent's output.
    pub color: String,
    /// Optional system prompt for this agent.
    pub system_prompt: Option<String>,
    /// Whether this agent can use tools.
    #[serde(default)]
    pub tools: bool,
    /// Whether to enable the provider's native web search.
    #[serde(default)]
    pub enable_web_search: bool,
    /// Optional sampling parameters for generation control.
    pub sampling: Option<SamplingConfig>,
    /// Whether to enable thinking/reasoning for this agent.
    #[serde(default)]
    pub enable_thinking: bool,
    /// Thinking effort level (Low/Medium/High). Only used when enable_thinking is true.
    pub thinking_effort: Option<ThinkingEffort>,
}

/// Sampling parameters for LLM generation.
/// All fields are optional; unset fields use provider defaults.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SamplingConfig {
    /// Sampling temperature. OpenAI/Google: 0-2, Anthropic: 0-1.
    pub temperature: Option<f64>,
    /// Nucleus sampling probability cutoff (0-1).
    pub top_p: Option<f64>,
    /// Top-K sampling. Only supported by Anthropic and Google.
    pub top_k: Option<u32>,
    /// Maximum output tokens. Defaults to model maximum.
    pub max_tokens: Option<u32>,
    /// Frequency penalty (-2.0 to 2.0). Only OpenAI Chat and Google.
    pub frequency_penalty: Option<f64>,
    /// Presence penalty (-2.0 to 2.0). Only OpenAI Chat and Google.
    pub presence_penalty: Option<f64>,
    /// Stop sequences to halt generation.
    pub stop_sequences: Option<Vec<String>>,
}

/// LLM provider SDK configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    /// Provider type: openai, anthropic, google.
    #[serde(rename = "type")]
    pub provider_type: ProviderType,
    /// API key value (not recommended; prefer `api_key_env`).
    pub api_key: Option<String>,
    /// Environment variable name holding the API key.
    pub api_key_env: Option<String>,
    /// Base URL for the provider API.
    pub base_url: Option<String>,
    /// Azure OpenAI endpoint URL (enables Azure mode when set).
    pub azure_endpoint: Option<String>,
    /// Azure OpenAI API version string.
    pub azure_api_version: Option<String>,
    /// Use the `name` field on messages to identify other agents.
    /// When false (default), other agents' content is prefixed with
    /// `[agent_name]` instead.
    #[serde(default)]
    pub use_name_field: bool,
    /// Google Vertex AI project ID (enables Vertex AI mode when set).
    pub vertex_project: Option<String>,
    /// Google Vertex AI location (e.g. "us-central1").
    pub vertex_location: Option<String>,
}

/// Supported LLM provider types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    /// OpenAI API (Chat Completions / Responses). Also covers Azure
    /// (via `azure_endpoint`) and OpenAI-compatible services (via `base_url`).
    OpenAI,
    /// Anthropic Messages API.
    Anthropic,
    /// Google Gemini API.
    Google,
}

/// MCP (Model Context Protocol) server configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct McpServerConfig {
    /// Server name for identification.
    pub name: String,
    /// Command to launch the MCP server process.
    pub command: String,
    /// Command-line arguments for the server process.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables passed to the server process.
    pub env: Option<HashMap<String, String>>,
    /// Trust level controlling tool approval behavior.
    pub trust: Option<McpTrust>,
}

/// Tool approval policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalMode {
    /// Read ops auto, write/shell/MCP require confirmation.
    #[default]
    Suggest,
    /// Read and write ops auto, shell/MCP require confirmation.
    AutoEdit,
    /// All operations execute without confirmation.
    FullAuto,
}

impl std::fmt::Display for ApprovalMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Suggest => write!(f, "suggest"),
            Self::AutoEdit => write!(f, "auto-edit"),
            Self::FullAuto => write!(f, "full-auto"),
        }
    }
}

impl std::str::FromStr for ApprovalMode {
    type Err = ConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "suggest" => Ok(Self::Suggest),
            "auto-edit" => Ok(Self::AutoEdit),
            "full-auto" => Ok(Self::FullAuto),
            _ => Err(ConfigError::Validation(format!(
                "invalid approval mode \"{s}\": valid options are suggest, auto-edit, full-auto"
            ))),
        }
    }
}

/// OpenAI API type selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiType {
    /// OpenAI Responses API (`POST /v1/responses`).
    Responses,
    /// OpenAI Chat Completions API (`POST /v1/chat/completions`).
    Chat,
}

/// Thinking/reasoning effort level for LLM providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingEffort {
    Low,
    Medium,
    High,
}

/// MCP server trust level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpTrust {
    /// Skip approval for this server's tools.
    Auto,
    /// Apply approval_mode rules (default).
    #[default]
    Confirm,
}

impl Config {
    /// Load configuration from a TOML file.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    /// Validate internal consistency of the configuration.
    ///
    /// Checks:
    /// - No duplicate agent names
    /// - Every name in `reply_order` exists in `agents`
    /// - Every agent's `provider` exists in `providers` (except `"builtin"`)
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Check for duplicate agent names.
        let mut seen = std::collections::HashSet::new();
        for agent in &self.agents {
            if !seen.insert(&agent.name) {
                return Err(ConfigError::Validation(format!(
                    "duplicate agent name: \"{}\"",
                    agent.name
                )));
            }
        }

        // Check reply_order references.
        for name in &self.settings.reply_order {
            if !self.agents.iter().any(|a| &a.name == name) {
                return Err(ConfigError::Validation(format!(
                    "reply_order references unknown agent: \"{name}\""
                )));
            }
        }

        // Check provider references.
        for agent in &self.agents {
            if agent.provider != "builtin" && !self.providers.contains_key(&agent.provider) {
                return Err(ConfigError::Validation(format!(
                    "agent \"{}\" references unknown provider: \"{}\"",
                    agent.name, agent.provider
                )));
            }
        }

        Ok(())
    }

    /// Normalize config: auto-append agents missing from `reply_order`.
    ///
    /// Returns a list of agent names that were appended (for warning display).
    pub fn normalize(&mut self) -> Vec<String> {
        let mut appended = Vec::new();
        for agent in &self.agents {
            if !self.settings.reply_order.contains(&agent.name) {
                appended.push(agent.name.clone());
                self.settings.reply_order.push(agent.name.clone());
            }
        }
        appended
    }

    /// Apply CLI argument overrides to the configuration.
    ///
    /// - `agents`: comma-separated agent names to keep (filters `self.agents`
    ///   and updates `reply_order`)
    /// - `approval_mode`: approval mode string to override `settings.approval_mode`
    pub fn apply_cli_overrides(
        &mut self,
        agents: Option<&str>,
        approval_mode: Option<&str>,
    ) -> Result<(), ConfigError> {
        if let Some(names) = agents {
            let requested: Vec<&str> = names.split(',').map(str::trim).collect();

            // Validate that all requested names exist.
            for name in &requested {
                if !self.agents.iter().any(|a| a.name == *name) {
                    return Err(ConfigError::Validation(format!(
                        "unknown agent specified via --agents: \"{name}\""
                    )));
                }
            }

            // Filter agents and update reply_order.
            self.agents.retain(|a| requested.contains(&a.name.as_str()));
            self.settings.reply_order = requested.iter().map(|s| s.to_string()).collect();
        }

        if let Some(mode_str) = approval_mode {
            self.settings.approval_mode = mode_str.parse()?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to deserialize a TOML value string into a type.
    #[derive(Deserialize)]
    struct Wrapper {
        val: ThinkingEffort,
    }

    #[test]
    fn thinking_effort_deserialize_low() {
        let w: Wrapper = toml::from_str("val = \"low\"").unwrap();
        assert_eq!(w.val, ThinkingEffort::Low);
    }

    #[test]
    fn thinking_effort_deserialize_medium() {
        let w: Wrapper = toml::from_str("val = \"medium\"").unwrap();
        assert_eq!(w.val, ThinkingEffort::Medium);
    }

    #[test]
    fn thinking_effort_deserialize_high() {
        let w: Wrapper = toml::from_str("val = \"high\"").unwrap();
        assert_eq!(w.val, ThinkingEffort::High);
    }

    #[test]
    fn thinking_effort_deserialize_invalid() {
        let result: Result<Wrapper, _> = toml::from_str("val = \"extreme\"");
        assert!(result.is_err());
    }

    #[test]
    fn agent_config_enable_thinking_default_false() {
        let toml_str = r#"
            name = "test"
            display_name = "Test"
            provider = "openai"
            model = "gpt-4"
            color = "blue"
        "#;
        let agent: AgentConfig = toml::from_str(toml_str).unwrap();
        assert!(!agent.enable_thinking);
    }

    #[test]
    fn agent_config_enable_thinking_with_effort() {
        let toml_str = r#"
            name = "test"
            display_name = "Test"
            provider = "openai"
            model = "gpt-4"
            color = "blue"
            enable_thinking = true
            thinking_effort = "high"
        "#;
        let agent: AgentConfig = toml::from_str(toml_str).unwrap();
        assert!(agent.enable_thinking);
        assert_eq!(agent.thinking_effort, Some(ThinkingEffort::High));
    }

    #[test]
    fn agent_config_enable_thinking_without_effort() {
        let toml_str = r#"
            name = "test"
            display_name = "Test"
            provider = "openai"
            model = "gpt-4"
            color = "blue"
            enable_thinking = true
        "#;
        let agent: AgentConfig = toml::from_str(toml_str).unwrap();
        assert!(agent.enable_thinking);
        assert!(agent.thinking_effort.is_none());
    }

    #[test]
    fn provider_config_vertex_fields() {
        let toml_str = r#"
            type = "google"
            api_key_env = "GOOGLE_API_KEY"
            vertex_project = "my-proj"
            vertex_location = "us-central1"
        "#;
        let provider: ProviderConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(provider.vertex_project.as_deref(), Some("my-proj"));
        assert_eq!(provider.vertex_location.as_deref(), Some("us-central1"));
    }

    #[test]
    fn provider_config_vertex_fields_missing() {
        let toml_str = r#"
            type = "google"
            api_key_env = "GOOGLE_API_KEY"
        "#;
        let provider: ProviderConfig = toml::from_str(toml_str).unwrap();
        assert!(provider.vertex_project.is_none());
        assert!(provider.vertex_location.is_none());
    }

    #[test]
    fn full_config_e2e_with_new_fields() {
        let toml_str = r#"
            [settings]
            approval_mode = "suggest"
            reply_order = ["agent1"]

            [[agents]]
            name = "agent1"
            display_name = "Agent 1"
            provider = "anthropic"
            model = "claude-opus-4-6"
            color = "green"
            enable_thinking = true
            thinking_effort = "medium"

            [providers.anthropic]
            type = "anthropic"
            api_key_env = "ANTHROPIC_API_KEY"

            [providers.google]
            type = "google"
            api_key_env = "GOOGLE_API_KEY"
            vertex_project = "my-proj"
            vertex_location = "us-central1"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        let agent = &config.agents[0];
        assert!(agent.enable_thinking);
        assert_eq!(agent.thinking_effort, Some(ThinkingEffort::Medium));

        let google = &config.providers["google"];
        assert_eq!(google.vertex_project.as_deref(), Some("my-proj"));
        assert_eq!(google.vertex_location.as_deref(), Some("us-central1"));
    }
}
