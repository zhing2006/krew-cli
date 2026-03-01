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

/// Global settings controlling session behavior.
#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    /// Tool approval policy: suggest, auto-edit, or full-auto.
    pub approval_mode: ApprovalMode,
    /// Agent reply order for `@all` broadcasts.
    pub reply_order: Vec<String>,
    /// Token threshold for auto-compact (0 or None = disabled).
    pub auto_compact_threshold: Option<u32>,
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
