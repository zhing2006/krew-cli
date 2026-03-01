use std::collections::HashMap;

use crate::{AgentConfig, ApprovalMode, Config, Settings};

/// Default auto-compact threshold in tokens.
pub const DEFAULT_AUTO_COMPACT_THRESHOLD: u32 = 120_000;

impl Default for Config {
    fn default() -> Self {
        Self {
            settings: Settings {
                approval_mode: ApprovalMode::Suggest,
                reply_order: vec!["echo".to_string()],
                auto_compact_threshold: None,
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
            }],
            providers: HashMap::new(),
            mcp_servers: Vec::new(),
        }
    }
}
