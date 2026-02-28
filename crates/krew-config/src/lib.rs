mod defaults;

pub use defaults::*;

use serde::Deserialize;
use std::collections::HashMap;

/// Root configuration loaded from `.krew/settings.toml`.
#[derive(Debug, Deserialize)]
pub struct Config {
    /// Global settings.
    pub settings: Settings,
    /// List of configured agents.
    pub agents: Vec<AgentConfig>,
    /// Provider SDK configurations keyed by provider name.
    pub providers: HashMap<String, ProviderConfig>,
    /// MCP server definitions.
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
}

/// Global settings controlling session behavior.
#[derive(Debug, Deserialize)]
pub struct Settings {
    /// Tool approval policy: suggest, auto-edit, or full-auto.
    pub approval_mode: ApprovalMode,
    /// Agent reply order for `@all` broadcasts.
    pub reply_order: Vec<String>,
    /// Token threshold for auto-compact (0 or None = disabled).
    pub auto_compact_threshold: Option<u32>,
}

/// Configuration for a single AI agent.
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Default, Deserialize)]
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
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Default, Deserialize)]
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

/// OpenAI API type selector.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiType {
    /// OpenAI Responses API (`POST /v1/responses`).
    Responses,
    /// OpenAI Chat Completions API (`POST /v1/chat/completions`).
    Chat,
}

/// MCP server trust level.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpTrust {
    /// Skip approval for this server's tools.
    Auto,
    /// Apply approval_mode rules (default).
    #[default]
    Confirm,
}
