use std::collections::HashMap;

use crate::{
    ApprovalMode, Config, DEFAULT_COMPACT_KEEP_ROUNDS, DEFAULT_INPUT_HISTORY_LIMIT,
    DEFAULT_SHELL_ALLOW_COMMANDS, DEFAULT_WORKER_THREADS, OtherAgentRole, RetryConfig, Settings,
};

/// Default auto-compact threshold in tokens.
pub const DEFAULT_AUTO_COMPACT_THRESHOLD: u32 = 120_000;

impl Default for Config {
    fn default() -> Self {
        Self {
            settings: Settings {
                approval_mode: ApprovalMode::Suggest,
                reply_order: Vec::new(),
                auto_compact_threshold: None,
                compact_keep_rounds: DEFAULT_COMPACT_KEEP_ROUNDS,
                input_history_limit: DEFAULT_INPUT_HISTORY_LIMIT,
                paste_burst_detection: true,
                worker_threads: DEFAULT_WORKER_THREADS,
                other_agent_role: OtherAgentRole::User,
                retry: RetryConfig::default(),
                shell_allow_commands: DEFAULT_SHELL_ALLOW_COMMANDS
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                fetch_allow_domains: Vec::new(),
            },
            agents: Vec::new(),
            providers: HashMap::new(),
            mcp_servers: Vec::new(),
        }
    }
}
