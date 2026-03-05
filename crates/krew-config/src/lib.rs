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
    /// Number of conversation rounds to keep during compaction.
    #[serde(default = "default_compact_keep_rounds")]
    pub compact_keep_rounds: usize,
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
    /// How to present other agents' messages in the conversation history.
    #[serde(default)]
    pub other_agent_role: OtherAgentRole,
    /// Retry configuration for LLM API requests.
    #[serde(default)]
    pub retry: RetryConfig,
    /// Shell commands that are auto-approved without user confirmation.
    ///
    /// Entries are prefix-matched against extracted command prefixes:
    /// - `"ls"` matches all `ls` invocations
    /// - `"cargo"` matches `cargo build`, `cargo test`, etc.
    /// - `"cargo build"` matches only `cargo build` (not `cargo test`)
    /// - `"git status"` matches only `git status` (not `git push`)
    #[serde(default = "default_shell_allow_commands")]
    pub shell_allow_commands: Vec<String>,
    /// Domains that skip approval for the fetch_url tool.
    ///
    /// Entries are suffix-matched against the URL host:
    /// - `"github.com"` matches `github.com` and `docs.github.com`
    /// - `"docs.rs"` matches `docs.rs` and any subdomain
    #[serde(default)]
    pub fetch_allow_domains: Vec<String>,
}

/// Retry configuration for LLM API requests.
///
/// Controls retry behavior for rate-limited (429) and server error (5xx)
/// responses. Each error type tracks its own retry count independently.
///
/// Delay formula for 429: `backoff_base_secs × backoff_multiplier^(attempt-1)`
#[derive(Debug, Clone, Deserialize)]
pub struct RetryConfig {
    /// Maximum retries for 429 rate-limit responses.
    #[serde(default = "default_retry_max_rate_limit")]
    pub max_retries_rate_limit: u32,
    /// Maximum retries for 5xx server error responses.
    #[serde(default = "default_retry_max_server_error")]
    pub max_retries_server_error: u32,
    /// Base delay in seconds for exponential backoff (429).
    #[serde(default = "default_retry_backoff_base_secs")]
    pub backoff_base_secs: f64,
    /// Multiplier for exponential backoff (429).
    #[serde(default = "default_retry_backoff_multiplier")]
    pub backoff_multiplier: f64,
    /// Fixed retry interval in seconds for 5xx server errors.
    #[serde(default = "default_retry_server_error_interval_secs")]
    pub server_error_interval_secs: f64,
    /// Request timeout in seconds for first token / initial response.
    #[serde(default = "default_retry_request_timeout_secs")]
    pub request_timeout_secs: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries_rate_limit: DEFAULT_RETRY_MAX_RATE_LIMIT,
            max_retries_server_error: DEFAULT_RETRY_MAX_SERVER_ERROR,
            backoff_base_secs: DEFAULT_RETRY_BACKOFF_BASE_SECS,
            backoff_multiplier: DEFAULT_RETRY_BACKOFF_MULTIPLIER,
            server_error_interval_secs: DEFAULT_RETRY_SERVER_ERROR_INTERVAL_SECS,
            request_timeout_secs: DEFAULT_RETRY_REQUEST_TIMEOUT_SECS,
        }
    }
}

/// Default number of conversation rounds to keep during compaction.
pub const DEFAULT_COMPACT_KEEP_ROUNDS: usize = 10;

fn default_compact_keep_rounds() -> usize {
    DEFAULT_COMPACT_KEEP_ROUNDS
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

// ── Retry defaults ──────────────────────────────────────────────────

/// Default maximum retries for 429 rate limit responses.
pub const DEFAULT_RETRY_MAX_RATE_LIMIT: u32 = 3;
/// Default maximum retries for 5xx server error responses.
pub const DEFAULT_RETRY_MAX_SERVER_ERROR: u32 = 2;
/// Default base delay in seconds for exponential backoff (429).
pub const DEFAULT_RETRY_BACKOFF_BASE_SECS: f64 = 2.0;
/// Default multiplier for exponential backoff (429).
pub const DEFAULT_RETRY_BACKOFF_MULTIPLIER: f64 = 3.0;
/// Default fixed retry interval in seconds for 5xx server errors.
pub const DEFAULT_RETRY_SERVER_ERROR_INTERVAL_SECS: f64 = 2.0;
/// Default request timeout in seconds for first token.
pub const DEFAULT_RETRY_REQUEST_TIMEOUT_SECS: u64 = 60;

fn default_retry_max_rate_limit() -> u32 {
    DEFAULT_RETRY_MAX_RATE_LIMIT
}
fn default_retry_max_server_error() -> u32 {
    DEFAULT_RETRY_MAX_SERVER_ERROR
}
fn default_retry_backoff_base_secs() -> f64 {
    DEFAULT_RETRY_BACKOFF_BASE_SECS
}
fn default_retry_backoff_multiplier() -> f64 {
    DEFAULT_RETRY_BACKOFF_MULTIPLIER
}
fn default_retry_server_error_interval_secs() -> f64 {
    DEFAULT_RETRY_SERVER_ERROR_INTERVAL_SECS
}
fn default_retry_request_timeout_secs() -> u64 {
    DEFAULT_RETRY_REQUEST_TIMEOUT_SECS
}

/// Default shell commands that are auto-approved (read-only / side-effect-free).
pub const DEFAULT_SHELL_ALLOW_COMMANDS: &[&str] = &[
    "cat", "cd", "cut", "date", "df", "du", "echo", "env", "expr", "false", "file", "find", "grep",
    "head", "hostname", "id", "ls", "nl", "paste", "printenv", "pwd", "rev", "rg", "seq", "sort",
    "stat", "tail", "tr", "true", "uname", "uniq", "wc", "which", "whoami",
];

fn default_shell_allow_commands() -> Vec<String> {
    DEFAULT_SHELL_ALLOW_COMMANDS
        .iter()
        .map(|s| s.to_string())
        .collect()
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
    #[serde(default = "default_true")]
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
    /// Google Vertex AI project ID (enables Vertex AI mode when set).
    pub vertex_project: Option<String>,
    /// Google Vertex AI location (e.g. "us-central1").
    pub vertex_location: Option<String>,
}

/// How to present other agents' messages in the conversation history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OtherAgentRole {
    /// Send other agents' replies as user-role messages (default).
    #[default]
    User,
    /// Send other agents' replies as assistant-role messages.
    Assistant,
}

/// Supported LLM provider types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    /// OpenAI API (Chat Completions / Responses). Also covers Azure
    /// and OpenAI-compatible services (via `base_url`).
    OpenAI,
    /// Anthropic Messages API.
    Anthropic,
    /// Google Gemini API.
    Google,
}

/// MCP (Model Context Protocol) server configuration.
///
/// Supports two transport modes:
/// - **stdio**: Set `command` (and optionally `args` / `env`) to spawn a child process.
/// - **HTTP**: Set `url` (and optionally `headers`) to connect via Streamable HTTP.
#[derive(Debug, Clone, Deserialize)]
pub struct McpServerConfig {
    /// Server name for identification.
    pub name: String,
    /// Command to launch the MCP server process (stdio transport).
    pub command: Option<String>,
    /// Command-line arguments for the server process (stdio transport).
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables passed to the server process (stdio transport).
    pub env: Option<HashMap<String, String>>,
    /// HTTP endpoint URL for Streamable HTTP transport.
    pub url: Option<String>,
    /// HTTP headers sent with every request (HTTP transport).
    pub headers: Option<HashMap<String, String>>,
    /// Trust level controlling tool approval behavior.
    pub trust: Option<McpTrust>,
}

impl McpServerConfig {
    /// Returns true if this server uses HTTP transport.
    pub fn is_http(&self) -> bool {
        self.url.is_some()
    }
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
