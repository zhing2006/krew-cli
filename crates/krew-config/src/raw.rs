//! Raw/partial configuration types for layered config merging.
//!
//! `RawConfig` and `UserConfig` preserve field presence (via `Option`) so that
//! user-level and project-level configs can be merged before resolving defaults.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::{
    AgentConfig, AgentToAgentRouting, ApprovalMode, Config, ConfigError,
    DEFAULT_AGENT_TO_AGENT_MAX_ROUNDS, DEFAULT_COMPACT_KEEP_ROUNDS, DEFAULT_INPUT_HISTORY_LIMIT,
    DEFAULT_SHELL_ALLOW_COMMANDS, DEFAULT_WORKER_THREADS, McpServerConfig, OtherAgentRole,
    ProviderConfig, RetryConfig, Settings, SkillsConfig,
};

/// User-level config directory name (relative to home).
pub const USER_CONFIG_DIR: &str = ".krew";

// ── RawSettings ─────────────────────────────────────────────────────

/// Raw settings with all scalar fields as `Option` to preserve
/// explicit-vs-default distinction during merge.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawSettings {
    pub approval_mode: Option<ApprovalMode>,
    #[serde(default)]
    pub reply_order: Vec<String>,
    pub auto_compact_threshold: Option<u32>,
    pub compact_keep_rounds: Option<usize>,
    pub input_history_limit: Option<usize>,
    pub paste_burst_detection: Option<bool>,
    pub worker_threads: Option<usize>,
    pub other_agent_role: Option<OtherAgentRole>,
    pub retry: Option<RetryConfig>,
    pub shell_allow_commands: Option<Vec<String>>,
    pub fetch_allow_domains: Option<Vec<String>>,
    pub agent_to_agent_routing: Option<AgentToAgentRouting>,
    pub agent_to_agent_max_rounds: Option<u32>,
    pub language: Option<String>,
}

// ── RawConfig ───────────────────────────────────────────────────────

/// Project-level raw configuration — settings fields are `Option` to preserve
/// explicit-vs-default distinction during merge with user config.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawConfig {
    #[serde(default)]
    pub settings: RawSettings,
    #[serde(default)]
    pub agents: Vec<AgentConfig>,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
    #[serde(default)]
    pub skills: Option<SkillsConfig>,
}

impl RawConfig {
    /// Load raw configuration from a TOML file (preserving field presence).
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    /// Merge user-level config into this project-level config.
    ///
    /// Merge rules:
    /// - **providers**: user as base, project same-key replaces entirely
    /// - **mcp_servers**: user first, project appended; same-name uses project's
    /// - **settings scalars**: project `Some` wins; project `None` inherits user
    /// - **skills**: project `Some` wins; project `None` inherits user
    pub fn merge_user(&mut self, user: &UserConfig) {
        // ── providers: user as base, project overrides by key ──
        let mut merged_providers = user.providers.clone();
        for (key, val) in self.providers.drain() {
            merged_providers.insert(key, val);
        }
        self.providers = merged_providers;

        // ── mcp_servers: user first, project appended, same-name dedup ──
        let project_servers = std::mem::take(&mut self.mcp_servers);
        let project_names: std::collections::HashSet<&str> =
            project_servers.iter().map(|s| s.name.as_str()).collect();
        // Keep user servers whose name is NOT overridden by project.
        let mut merged_servers: Vec<McpServerConfig> = user
            .mcp_servers
            .iter()
            .filter(|s| !project_names.contains(s.name.as_str()))
            .cloned()
            .collect();
        merged_servers.extend(project_servers);
        self.mcp_servers = merged_servers;

        // ── settings scalars: project Some wins, else inherit user ──
        macro_rules! merge_option {
            ($field:ident) => {
                if self.settings.$field.is_none() {
                    self.settings.$field = user.settings.$field.clone();
                }
            };
        }
        merge_option!(approval_mode);
        merge_option!(auto_compact_threshold);
        merge_option!(compact_keep_rounds);
        merge_option!(input_history_limit);
        merge_option!(paste_burst_detection);
        merge_option!(worker_threads);
        merge_option!(other_agent_role);
        merge_option!(retry);
        merge_option!(shell_allow_commands);
        merge_option!(fetch_allow_domains);
        merge_option!(agent_to_agent_routing);
        merge_option!(agent_to_agent_max_rounds);
        merge_option!(language);

        // ── skills: project Some wins, else inherit user ──
        if self.skills.is_none() {
            self.skills = user.skills.clone();
        }
    }

    /// Resolve all `Option` fields to their defaults, producing the final `Config`.
    pub fn resolve(self) -> Config {
        let shell_allow = self.settings.shell_allow_commands.unwrap_or_else(|| {
            DEFAULT_SHELL_ALLOW_COMMANDS
                .iter()
                .map(|s| s.to_string())
                .collect()
        });

        Config {
            settings: Settings {
                approval_mode: self.settings.approval_mode.unwrap_or_default(),
                reply_order: self.settings.reply_order,
                auto_compact_threshold: self.settings.auto_compact_threshold,
                compact_keep_rounds: self
                    .settings
                    .compact_keep_rounds
                    .unwrap_or(DEFAULT_COMPACT_KEEP_ROUNDS),
                input_history_limit: self
                    .settings
                    .input_history_limit
                    .unwrap_or(DEFAULT_INPUT_HISTORY_LIMIT),
                paste_burst_detection: self.settings.paste_burst_detection.unwrap_or(true),
                worker_threads: self
                    .settings
                    .worker_threads
                    .unwrap_or(DEFAULT_WORKER_THREADS),
                other_agent_role: self.settings.other_agent_role.unwrap_or_default(),
                retry: self.settings.retry.unwrap_or_default(),
                shell_allow_commands: shell_allow,
                fetch_allow_domains: self.settings.fetch_allow_domains.unwrap_or_default(),
                agent_to_agent_routing: self.settings.agent_to_agent_routing.unwrap_or_default(),
                agent_to_agent_max_rounds: self
                    .settings
                    .agent_to_agent_max_rounds
                    .unwrap_or(DEFAULT_AGENT_TO_AGENT_MAX_ROUNDS),
                language: self.settings.language,
            },
            agents: self.agents,
            providers: self.providers,
            mcp_servers: self.mcp_servers,
            skills: self.skills.unwrap_or_default(),
        }
    }
}

// ── UserSettings ────────────────────────────────────────────────────

/// User-level settings — same shape as `RawSettings` minus `reply_order`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct UserSettings {
    pub approval_mode: Option<ApprovalMode>,
    pub auto_compact_threshold: Option<u32>,
    pub compact_keep_rounds: Option<usize>,
    pub input_history_limit: Option<usize>,
    pub paste_burst_detection: Option<bool>,
    pub worker_threads: Option<usize>,
    pub other_agent_role: Option<OtherAgentRole>,
    pub retry: Option<RetryConfig>,
    pub shell_allow_commands: Option<Vec<String>>,
    pub fetch_allow_domains: Option<Vec<String>>,
    pub agent_to_agent_routing: Option<AgentToAgentRouting>,
    pub agent_to_agent_max_rounds: Option<u32>,
    pub language: Option<String>,
}

// ── UserConfig ──────────────────────────────────────────────────────

/// User-level configuration loaded from `~/.krew/settings.toml`.
///
/// All fields are optional — only specified fields participate in merging.
/// Does not contain `agents` or `reply_order` (project-only concerns).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct UserConfig {
    #[serde(default)]
    pub settings: UserSettings,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
    #[serde(default)]
    pub skills: Option<SkillsConfig>,
}

impl UserConfig {
    /// Load user-level configuration from `~/.krew/settings.toml`.
    ///
    /// - File does not exist → returns `UserConfig::default()` silently.
    /// - File exists but parse fails → prints terminal warning via `eprintln!`,
    ///   returns `UserConfig::default()`.
    pub fn load() -> Self {
        let Some(home) = dirs_home() else {
            return Self::default();
        };
        let path = home.join(USER_CONFIG_DIR).join("settings.toml");
        if !path.exists() {
            return Self::default();
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str::<Self>(&content) {
                Ok(cfg) => {
                    tracing::info!(path = %path.display(), "Loaded user config");
                    cfg
                }
                Err(e) => {
                    eprintln!(
                        "Warning: failed to parse {}: {e}. Using default config.",
                        path.display()
                    );
                    Self::default()
                }
            },
            Err(e) => {
                eprintln!(
                    "Warning: failed to read {}: {e}. Using default config.",
                    path.display()
                );
                Self::default()
            }
        }
    }
}

/// Get the user's home directory.
fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}
