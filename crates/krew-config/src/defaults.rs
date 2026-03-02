use std::collections::HashMap;

use crate::{
    AgentConfig, ApprovalMode, Config, DEFAULT_INPUT_HISTORY_LIMIT, DEFAULT_WORKER_THREADS,
    OtherAgentRole, RetryConfig, Settings,
};

/// Default auto-compact threshold in tokens.
pub const DEFAULT_AUTO_COMPACT_THRESHOLD: u32 = 120_000;

impl Default for Config {
    fn default() -> Self {
        Self {
            settings: Settings {
                approval_mode: ApprovalMode::Suggest,
                reply_order: vec!["echo".to_string()],
                auto_compact_threshold: None,
                input_history_limit: DEFAULT_INPUT_HISTORY_LIMIT,
                paste_burst_detection: true,
                worker_threads: DEFAULT_WORKER_THREADS,
                other_agent_role: OtherAgentRole::User,
                retry: RetryConfig::default(),
            },
            agents: vec![AgentConfig {
                name: "echo".to_string(),
                display_name: "Echo".to_string(),
                provider: "builtin".to_string(),
                model: "echo".to_string(),
                api_type: None,
                color: "yellow".to_string(),
                system_prompt: None,
                tools: false,
                enable_web_search: false,
                sampling: None,
                enable_thinking: false,
                thinking_effort: None,
            }],
            providers: HashMap::new(),
            mcp_servers: Vec::new(),
        }
    }
}
