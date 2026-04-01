use std::collections::HashMap;
use std::io::Write;

use tempfile::NamedTempFile;

use krew_config::{
    ApprovalMode, Config, ConfigError, DEFAULT_COMPACT_KEEP_ROUNDS, DEFAULT_INPUT_HISTORY_LIMIT,
    DEFAULT_WORKER_THREADS, McpServerConfig, ProviderConfig, RawConfig, SkillsConfig, UserConfig,
    UserSettings,
};

// ── Helper: minimal valid TOML for RawConfig ────────────────────────

const VALID_RAW_TOML: &str = r#"
[settings]
approval_mode = "suggest"
reply_order = ["gpt"]

[[agents]]
name = "gpt"
display_name = "GPT"
provider = "openai"
model = "gpt-5"
color = "green"

[providers.openai]
type = "openai"
api_key_env = "OPENAI_API_KEY"
"#;

fn make_provider(ptype: &str, key_env: &str) -> ProviderConfig {
    toml::from_str(&format!(
        r#"type = "{ptype}"
api_key_env = "{key_env}""#
    ))
    .unwrap()
}

fn make_mcp(name: &str) -> McpServerConfig {
    toml::from_str(&format!(r#"name = "{name}""#)).unwrap()
}

// ── 2.5: resolve fills defaults for None fields ─────────────────────

#[test]
fn resolve_none_fields_get_defaults() {
    let raw = RawConfig::default();
    let config = raw.resolve();
    assert_eq!(config.settings.approval_mode, ApprovalMode::Suggest);
    assert_eq!(config.settings.worker_threads, DEFAULT_WORKER_THREADS);
    assert_eq!(
        config.settings.compact_keep_rounds,
        DEFAULT_COMPACT_KEEP_ROUNDS
    );
    assert_eq!(
        config.settings.input_history_limit,
        DEFAULT_INPUT_HISTORY_LIMIT
    );
    assert!(config.settings.paste_burst_detection);
    assert!(config.settings.auto_compact_threshold.is_none());
    assert!(config.allow_rules.is_empty());
    assert!(config.deny_rules.is_empty());
    assert!(config.ask_rules.is_empty());
    assert!(config.agents.is_empty());
    assert!(config.providers.is_empty());
    assert!(config.skills.enabled);
}

// ── 2.6: resolve preserves Some fields ──────────────────────────────

#[test]
fn resolve_some_fields_preserved() {
    let mut raw = RawConfig::default();
    raw.settings.approval_mode = Some(ApprovalMode::FullAuto);
    raw.settings.worker_threads = Some(16);
    raw.settings.paste_burst_detection = Some(false);
    raw.allow_rules = vec![krew_config::PermissionRule {
        tool: "shell".into(),
        pattern: Some("cargo *".into()),
        reason: None,
    }];
    raw.skills = Some(SkillsConfig {
        enabled: false,
        extra_paths: vec!["/custom".to_string()],
    });

    let config = raw.resolve();
    assert_eq!(config.settings.approval_mode, ApprovalMode::FullAuto);
    assert_eq!(config.settings.worker_threads, 16);
    assert!(!config.settings.paste_burst_detection);
    assert_eq!(config.allow_rules.len(), 1);
    assert_eq!(config.allow_rules[0].tool, "shell");
    assert!(!config.skills.enabled);
    assert_eq!(config.skills.extra_paths, vec!["/custom"]);
}

// ── 2.7: Config::load() still works after refactor ──────────────────

#[test]
fn config_load_still_works() {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(VALID_RAW_TOML.as_bytes()).unwrap();
    let config = Config::load(f.path()).unwrap();
    assert_eq!(config.agents.len(), 1);
    assert_eq!(config.agents[0].name, "gpt");
    assert_eq!(config.settings.approval_mode, ApprovalMode::Suggest);
}

#[test]
fn config_load_file_not_found() {
    let result = Config::load(std::path::Path::new("/nonexistent"));
    assert!(matches!(result.unwrap_err(), ConfigError::Io(_)));
}

#[test]
fn config_load_invalid_toml() {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(b"[[ broken").unwrap();
    let result = Config::load(f.path());
    assert!(matches!(result.unwrap_err(), ConfigError::Parse(_)));
}

// ── 2.8: RawConfig preserves field presence ─────────────────────────

#[test]
fn raw_config_preserves_field_presence() {
    let toml = r#"
[settings]
approval_mode = "full-auto"

[[agents]]
name = "a"
display_name = "A"
provider = "builtin"
model = "echo"
color = "red"
"#;
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(toml.as_bytes()).unwrap();
    let raw = RawConfig::load(f.path()).unwrap();
    assert_eq!(raw.settings.approval_mode, Some(ApprovalMode::FullAuto));
    assert!(raw.settings.worker_threads.is_none());
    assert!(raw.settings.compact_keep_rounds.is_none());
    assert!(raw.allow_rules.is_empty());
    assert!(raw.settings.other_agent_role.is_none());
    assert!(raw.settings.retry.is_none());
}

// ── 3.2: UserConfig from valid TOML ─────────────────────────────────

#[test]
fn user_config_from_valid_toml() {
    let toml = r#"
[settings]
approval_mode = "full-auto"
worker_threads = 8

[providers.openai]
type = "openai"
api_key_env = "OPENAI_API_KEY"

[[mcp_servers]]
name = "global-mcp"
url = "http://localhost:8080/mcp"
"#;
    let cfg: UserConfig = toml::from_str(toml).unwrap();
    assert_eq!(cfg.settings.approval_mode, Some(ApprovalMode::FullAuto));
    assert_eq!(cfg.settings.worker_threads, Some(8));
    assert!(cfg.providers.contains_key("openai"));
    assert_eq!(cfg.mcp_servers.len(), 1);
    assert_eq!(cfg.mcp_servers[0].name, "global-mcp");
}

// ── 3.3: UserConfig default for empty ───────────────────────────────

#[test]
fn user_config_default_for_empty_toml() {
    let cfg: UserConfig = toml::from_str("").unwrap();
    assert!(cfg.settings.approval_mode.is_none());
    assert!(cfg.providers.is_empty());
    assert!(cfg.mcp_servers.is_empty());
    assert!(cfg.skills.is_none());
}

// ── 3.4: UserConfig ignores unknown fields ──────────────────────────

#[test]
fn user_config_ignores_agents() {
    // UserConfig has no agents field — should not fail on unknown keys.
    // toml crate by default ignores unknown fields with Deserialize.
    let toml = r#"
[settings]
approval_mode = "suggest"
"#;
    let cfg: UserConfig = toml::from_str(toml).unwrap();
    assert_eq!(cfg.settings.approval_mode, Some(ApprovalMode::Suggest));
}

// ── 4.2: providers merge — project overrides same key ───────────────

#[test]
fn merge_providers_project_overrides() {
    let user = UserConfig {
        providers: HashMap::from([
            ("openai".into(), make_provider("openai", "USER_KEY")),
            ("anthropic".into(), make_provider("anthropic", "USER_ANTH")),
        ]),
        ..Default::default()
    };
    let mut raw = RawConfig {
        providers: HashMap::from([("openai".into(), make_provider("openai", "PROJECT_KEY"))]),
        ..Default::default()
    };
    raw.merge_user(&user);

    assert_eq!(raw.providers.len(), 2);
    assert_eq!(
        raw.providers["openai"].api_key_env.as_deref(),
        Some("PROJECT_KEY")
    );
    assert_eq!(
        raw.providers["anthropic"].api_key_env.as_deref(),
        Some("USER_ANTH")
    );
}

// ── 4.3: providers — user only ──────────────────────────────────────

#[test]
fn merge_providers_user_only() {
    let user = UserConfig {
        providers: HashMap::from([("google".into(), make_provider("google", "GOOGLE_KEY"))]),
        ..Default::default()
    };
    let mut raw = RawConfig::default();
    raw.merge_user(&user);

    assert_eq!(raw.providers.len(), 1);
    assert!(raw.providers.contains_key("google"));
}

// ── 4.4: providers — both empty ─────────────────────────────────────

#[test]
fn merge_providers_both_empty() {
    let user = UserConfig::default();
    let mut raw = RawConfig::default();
    raw.merge_user(&user);
    assert!(raw.providers.is_empty());
}

// ── 4.5: mcp_servers merge with dedup ───────────────────────────────

#[test]
fn merge_mcp_servers_dedup() {
    let user = UserConfig {
        mcp_servers: vec![make_mcp("A"), make_mcp("B")],
        ..Default::default()
    };
    let mut raw = RawConfig {
        mcp_servers: vec![make_mcp("B"), make_mcp("C")],
        ..Default::default()
    };
    raw.merge_user(&user);

    let names: Vec<&str> = raw.mcp_servers.iter().map(|s| s.name.as_str()).collect();
    // A (user, not overridden), B (project), C (project)
    assert_eq!(names, vec!["A", "B", "C"]);
}

// ── 4.6: mcp_servers — user only ────────────────────────────────────

#[test]
fn merge_mcp_servers_user_only() {
    let user = UserConfig {
        mcp_servers: vec![make_mcp("global")],
        ..Default::default()
    };
    let mut raw = RawConfig::default();
    raw.merge_user(&user);

    assert_eq!(raw.mcp_servers.len(), 1);
    assert_eq!(raw.mcp_servers[0].name, "global");
}

// ── 4.7: settings — project Some wins ───────────────────────────────

#[test]
fn merge_settings_project_some_wins() {
    let user = UserConfig {
        settings: UserSettings {
            approval_mode: Some(ApprovalMode::FullAuto),
            worker_threads: Some(16),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut raw = RawConfig {
        settings: krew_config::RawSettings {
            approval_mode: Some(ApprovalMode::Suggest),
            worker_threads: Some(4),
            ..Default::default()
        },
        ..Default::default()
    };
    raw.merge_user(&user);

    assert_eq!(raw.settings.approval_mode, Some(ApprovalMode::Suggest));
    assert_eq!(raw.settings.worker_threads, Some(4));
}

// ── 4.8: settings — project None inherits user Some ─────────────────

#[test]
fn merge_settings_project_none_inherits_user() {
    let user = UserConfig {
        settings: UserSettings {
            approval_mode: Some(ApprovalMode::FullAuto),
            worker_threads: Some(8),
            paste_burst_detection: Some(false),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut raw = RawConfig::default();
    raw.merge_user(&user);

    assert_eq!(raw.settings.approval_mode, Some(ApprovalMode::FullAuto));
    assert_eq!(raw.settings.worker_threads, Some(8));
    assert_eq!(raw.settings.paste_burst_detection, Some(false));
}

// ── 4.9: settings — both None stays None ────────────────────────────

#[test]
fn merge_settings_both_none_stays_none() {
    let user = UserConfig::default();
    let mut raw = RawConfig::default();
    raw.merge_user(&user);

    assert!(raw.settings.approval_mode.is_none());
    assert!(raw.settings.worker_threads.is_none());
    assert!(raw.settings.compact_keep_rounds.is_none());
}

// ── 4.10: skills — project Some wins ────────────────────────────────

#[test]
fn merge_skills_project_some_wins() {
    let user = UserConfig {
        skills: Some(SkillsConfig {
            enabled: false,
            extra_paths: vec!["/user-path".into()],
        }),
        ..Default::default()
    };
    let mut raw = RawConfig {
        skills: Some(SkillsConfig {
            enabled: true,
            extra_paths: vec!["/project-path".into()],
        }),
        ..Default::default()
    };
    raw.merge_user(&user);

    let skills = raw.skills.as_ref().unwrap();
    assert!(skills.enabled);
    assert_eq!(skills.extra_paths, vec!["/project-path"]);
}

// ── 4.11: skills — project None inherits user ───────────────────────

#[test]
fn merge_skills_project_none_inherits_user() {
    let user = UserConfig {
        skills: Some(SkillsConfig {
            enabled: false,
            extra_paths: vec!["/user-path".into()],
        }),
        ..Default::default()
    };
    let mut raw = RawConfig::default(); // skills = None
    raw.merge_user(&user);

    let skills = raw.skills.as_ref().unwrap();
    assert!(!skills.enabled);
    assert_eq!(skills.extra_paths, vec!["/user-path"]);
}

// ── 4.12: end-to-end merge + resolve ────────────────────────────────

#[test]
fn merge_resolve_end_to_end() {
    // User config: provides API keys and some settings.
    let user = UserConfig {
        settings: UserSettings {
            approval_mode: Some(ApprovalMode::FullAuto),
            worker_threads: Some(8),
            ..Default::default()
        },
        providers: HashMap::from([
            ("openai".into(), make_provider("openai", "USER_OAI")),
            ("anthropic".into(), make_provider("anthropic", "USER_ANTH")),
        ]),
        mcp_servers: vec![make_mcp("global-mcp")],
        skills: Some(SkillsConfig {
            enabled: true,
            extra_paths: vec!["/user-skills".into()],
        }),
        allow_rules: Vec::new(),
        deny_rules: Vec::new(),
        ask_rules: Vec::new(),
    };

    // Project config: overrides approval_mode, adds agent, overrides openai provider.
    let project_toml = r#"
[settings]
approval_mode = "suggest"
reply_order = ["gpt"]

[[agents]]
name = "gpt"
display_name = "GPT"
provider = "openai"
model = "gpt-5"
color = "green"

[providers.openai]
type = "openai"
api_key_env = "PROJECT_OAI"
"#;
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(project_toml.as_bytes()).unwrap();
    let mut raw = RawConfig::load(f.path()).unwrap();
    raw.merge_user(&user);

    let config = raw.resolve();

    // approval_mode: project "suggest" wins over user "full-auto".
    assert_eq!(config.settings.approval_mode, ApprovalMode::Suggest);
    // worker_threads: project None → inherits user's 8.
    assert_eq!(config.settings.worker_threads, 8);
    // providers: openai uses project's key, anthropic from user.
    assert_eq!(
        config.providers["openai"].api_key_env.as_deref(),
        Some("PROJECT_OAI")
    );
    assert_eq!(
        config.providers["anthropic"].api_key_env.as_deref(),
        Some("USER_ANTH")
    );
    // mcp_servers: user's global-mcp included.
    assert!(config.mcp_servers.iter().any(|s| s.name == "global-mcp"));
    // agents from project.
    assert_eq!(config.agents.len(), 1);
    assert_eq!(config.agents[0].name, "gpt");
    // skills: project has no [skills], inherits user's.
    assert!(config.skills.enabled);
    assert_eq!(config.skills.extra_paths, vec!["/user-skills"]);
}

// ── language setting ─────────────────────────────────────────────────

#[test]
fn resolve_language_default_is_none() {
    let raw = RawConfig::default();
    let config = raw.resolve();
    assert!(config.settings.language.is_none());
}

#[test]
fn resolve_language_preserves_value() {
    let mut raw = RawConfig::default();
    raw.settings.language = Some("中文".to_string());
    let config = raw.resolve();
    assert_eq!(config.settings.language.as_deref(), Some("中文"));
}

#[test]
fn merge_language_project_overrides_user() {
    let user = UserConfig {
        settings: UserSettings {
            language: Some("English".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut raw = RawConfig {
        settings: krew_config::RawSettings {
            language: Some("中文".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    raw.merge_user(&user);
    assert_eq!(raw.settings.language.as_deref(), Some("中文"));
}

// ── restrict_workspace merge & resolve ───────────────────────────

#[test]
fn merge_restrict_workspace_project_true_wins_over_user_false() {
    let user = UserConfig {
        settings: UserSettings {
            restrict_workspace: Some(false),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut raw = RawConfig {
        settings: krew_config::RawSettings {
            restrict_workspace: Some(true),
            ..Default::default()
        },
        ..Default::default()
    };
    raw.merge_user(&user);
    let config = raw.resolve();
    assert!(config.settings.restrict_workspace);
}

#[test]
fn merge_restrict_workspace_project_none_inherits_user_false() {
    let user = UserConfig {
        settings: UserSettings {
            restrict_workspace: Some(false),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut raw = RawConfig::default();
    raw.merge_user(&user);
    let config = raw.resolve();
    assert!(!config.settings.restrict_workspace);
}

#[test]
fn resolve_restrict_workspace_default_is_true() {
    let raw = RawConfig::default();
    let config = raw.resolve();
    assert!(config.settings.restrict_workspace);
}

#[test]
fn merge_language_project_none_inherits_user() {
    let user = UserConfig {
        settings: UserSettings {
            language: Some("日本語".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut raw = RawConfig::default();
    raw.merge_user(&user);
    assert_eq!(raw.settings.language.as_deref(), Some("日本語"));
}

// ── update_check merge & resolve ────────────────────────────────

#[test]
fn resolve_update_check_default_is_true() {
    let raw = RawConfig::default();
    let config = raw.resolve();
    assert!(config.settings.update_check);
}

#[test]
fn merge_update_check_project_false_wins() {
    let user = UserConfig {
        settings: UserSettings {
            update_check: Some(true),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut raw = RawConfig {
        settings: krew_config::RawSettings {
            update_check: Some(false),
            ..Default::default()
        },
        ..Default::default()
    };
    raw.merge_user(&user);
    let config = raw.resolve();
    assert!(!config.settings.update_check);
}

#[test]
fn merge_update_check_project_none_inherits_user_false() {
    let user = UserConfig {
        settings: UserSettings {
            update_check: Some(false),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut raw = RawConfig::default();
    raw.merge_user(&user);
    let config = raw.resolve();
    assert!(!config.settings.update_check);
}
