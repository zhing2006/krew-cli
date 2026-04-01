use std::collections::HashMap;

use crate::{
    AgentToAgentRouting, ApprovalMode, Config, DEFAULT_AGENT_TO_AGENT_MAX_ROUNDS,
    DEFAULT_COMPACT_KEEP_ROUNDS, DEFAULT_INPUT_HISTORY_LIMIT, DEFAULT_WORKER_THREADS,
    OtherAgentRole, RetryConfig, Settings, SkillsConfig,
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
                agent_to_agent_routing: AgentToAgentRouting::Immediate,
                agent_to_agent_max_rounds: DEFAULT_AGENT_TO_AGENT_MAX_ROUNDS,
                language: None,
                restrict_workspace: true,
                sub_agent_enabled: false,
                update_check: true,
            },
            agents: Vec::new(),
            providers: HashMap::new(),
            mcp_servers: Vec::new(),
            skills: SkillsConfig::default(),
            allow_rules: Vec::new(),
            deny_rules: Vec::new(),
            ask_rules: Vec::new(),
        }
    }
}
