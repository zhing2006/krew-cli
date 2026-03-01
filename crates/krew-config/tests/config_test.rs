use std::io::Write;
use tempfile::NamedTempFile;

use krew_config::{ApprovalMode, Config, ConfigError};

const VALID_CONFIG: &str = r#"
[settings]
approval_mode = "suggest"
reply_order = ["gpt", "opus"]

[[agents]]
name = "gpt"
display_name = "GPT-5.2"
provider = "openai"
model = "gpt-5.2"
color = "green"

[[agents]]
name = "opus"
display_name = "Claude Opus"
provider = "anthropic"
model = "claude-opus-4-6"
color = "magenta"

[providers.openai]
type = "openai"
api_key_env = "OPENAI_API_KEY"

[providers.anthropic]
type = "anthropic"
api_key_env = "ANTHROPIC_API_KEY"
"#;

// ── Config::load() ──────────────────────────────────────────────────────

#[test]
fn load_valid_config() {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(VALID_CONFIG.as_bytes()).unwrap();

    let config = Config::load(f.path()).unwrap();
    assert_eq!(config.agents.len(), 2);
    assert_eq!(config.agents[0].name, "gpt");
    assert_eq!(config.agents[1].name, "opus");
    assert_eq!(config.settings.reply_order, vec!["gpt", "opus"]);
    assert!(matches!(
        config.settings.approval_mode,
        ApprovalMode::Suggest
    ));
    assert_eq!(config.providers.len(), 2);
}

#[test]
fn load_file_not_found() {
    let result = Config::load(std::path::Path::new("/nonexistent/path/config.toml"));
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ConfigError::Io(_)));
}

#[test]
fn load_invalid_toml() {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(b"this is [[ not valid toml").unwrap();

    let result = Config::load(f.path());
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ConfigError::Parse(_)));
}

// ── Config::default() ───────────────────────────────────────────────────

#[test]
fn default_config_has_echo_agent() {
    let config = Config::default();
    assert_eq!(config.agents.len(), 1);
    assert_eq!(config.agents[0].name, "echo");
    assert_eq!(config.agents[0].display_name, "Echo");
    assert_eq!(config.agents[0].provider, "builtin");
    assert_eq!(config.settings.reply_order, vec!["echo"]);
    assert!(matches!(
        config.settings.approval_mode,
        ApprovalMode::Suggest
    ));
}

// ── Config::validate() ─────────────────────────────────────────────────

#[test]
fn validate_valid_config() {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(VALID_CONFIG.as_bytes()).unwrap();
    let config = Config::load(f.path()).unwrap();
    assert!(config.validate().is_ok());
}

#[test]
fn validate_default_config() {
    let config = Config::default();
    assert!(config.validate().is_ok());
}

#[test]
fn validate_invalid_reply_order() {
    let toml = r#"
[settings]
approval_mode = "suggest"
reply_order = ["gpt", "nonexistent"]

[[agents]]
name = "gpt"
display_name = "GPT"
provider = "openai"
model = "gpt-5"
color = "green"

[providers.openai]
type = "openai"
api_key_env = "KEY"
"#;
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(toml.as_bytes()).unwrap();
    let config = Config::load(f.path()).unwrap();
    let err = config.validate().unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("nonexistent"),
        "error should mention the invalid name: {msg}"
    );
}

#[test]
fn validate_invalid_provider_ref() {
    let toml = r#"
[settings]
approval_mode = "suggest"
reply_order = ["gpt"]

[[agents]]
name = "gpt"
display_name = "GPT"
provider = "missing_provider"
model = "gpt-5"
color = "green"
"#;
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(toml.as_bytes()).unwrap();
    let config = Config::load(f.path()).unwrap();
    let err = config.validate().unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("missing_provider"),
        "error should mention the invalid provider: {msg}"
    );
}

#[test]
fn validate_duplicate_agent_name() {
    let toml = r#"
[settings]
approval_mode = "suggest"
reply_order = ["gpt"]

[[agents]]
name = "gpt"
display_name = "GPT"
provider = "openai"
model = "gpt-5"
color = "green"

[[agents]]
name = "gpt"
display_name = "GPT Copy"
provider = "openai"
model = "gpt-5"
color = "blue"

[providers.openai]
type = "openai"
api_key_env = "KEY"
"#;
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(toml.as_bytes()).unwrap();
    let config = Config::load(f.path()).unwrap();
    let err = config.validate().unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("duplicate"),
        "error should mention duplicate: {msg}"
    );
}

#[test]
fn validate_builtin_provider_skipped() {
    // builtin provider should not require an entry in [providers]
    let config = Config::default(); // uses provider = "builtin"
    assert!(config.validate().is_ok());
}

// ── Config::apply_cli_overrides() ──────────────────────────────────────

fn load_test_config() -> Config {
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(VALID_CONFIG.as_bytes()).unwrap();
    Config::load(f.path()).unwrap()
}

#[test]
fn overrides_none_leaves_config_unchanged() {
    let mut config = load_test_config();
    let original_agents_len = config.agents.len();
    config.apply_cli_overrides(None, None).unwrap();
    assert_eq!(config.agents.len(), original_agents_len);
}

#[test]
fn overrides_agents_filters_list() {
    let mut config = load_test_config();
    config.apply_cli_overrides(Some("opus"), None).unwrap();
    assert_eq!(config.agents.len(), 1);
    assert_eq!(config.agents[0].name, "opus");
    assert_eq!(config.settings.reply_order, vec!["opus"]);
}

#[test]
fn overrides_agents_unknown_name() {
    let mut config = load_test_config();
    let result = config.apply_cli_overrides(Some("gpt,nonexistent"), None);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("nonexistent"),
        "error should mention the invalid name: {msg}"
    );
}

#[test]
fn overrides_approval_mode() {
    let mut config = load_test_config();
    config.apply_cli_overrides(None, Some("full-auto")).unwrap();
    assert_eq!(config.settings.approval_mode, ApprovalMode::FullAuto);
}

#[test]
fn overrides_invalid_approval_mode() {
    let mut config = load_test_config();
    let result = config.apply_cli_overrides(None, Some("invalid"));
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("suggest"),
        "error should list valid options: {msg}"
    );
}
