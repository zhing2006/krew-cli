//! Integration tests for `krew config` CLI subcommands.
//!
//! These tests verify clap parsing and end-to-end config file generation.

use std::process::Command;

fn krew_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_krew"))
}

// ── Clap parsing tests ──────────────────────────────────────────────

#[test]
fn config_init_parses() {
    let output = krew_bin()
        .args(["config", "init", "--help"])
        .output()
        .expect("failed to run krew");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--user") || stdout.contains("--project"));
}

#[test]
fn config_add_provider_parses() {
    let output = krew_bin()
        .args(["config", "add", "provider", "--help"])
        .output()
        .expect("failed to run krew");
    assert!(output.status.success());
}

#[test]
fn config_add_agent_parses() {
    let output = krew_bin()
        .args(["config", "add", "agent", "--help"])
        .output()
        .expect("failed to run krew");
    assert!(output.status.success());
}

#[test]
fn config_del_provider_parses() {
    let output = krew_bin()
        .args(["config", "del", "provider", "--help"])
        .output()
        .expect("failed to run krew");
    assert!(output.status.success());
}

#[test]
fn config_del_agent_parses() {
    let output = krew_bin()
        .args(["config", "del", "agent", "--help"])
        .output()
        .expect("failed to run krew");
    assert!(output.status.success());
}

#[test]
fn config_list_providers_parses() {
    let output = krew_bin()
        .args(["config", "list", "providers", "--help"])
        .output()
        .expect("failed to run krew");
    assert!(output.status.success());
}

#[test]
fn config_list_agents_parses() {
    let output = krew_bin()
        .args(["config", "list", "agents", "--help"])
        .output()
        .expect("failed to run krew");
    assert!(output.status.success());
}

#[test]
fn config_doctor_parses() {
    let output = krew_bin()
        .args(["config", "doctor", "--help"])
        .output()
        .expect("failed to run krew");
    assert!(output.status.success());
}

#[test]
fn config_init_user_project_mutually_exclusive() {
    let output = krew_bin()
        .args(["config", "init", "--user", "--project"])
        .output()
        .expect("failed to run krew");
    // Should fail with clap conflict error.
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot be used with") || stderr.contains("conflict"),
        "expected conflict error, got: {stderr}"
    );
}

// ── End-to-end: generated config loads correctly ────────────────────

#[test]
fn generated_config_loads_and_validates() {
    use krew_config::RawConfig;

    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("settings.toml");

    // Write a config file matching what the wizard would generate.
    // Note: wizard generates minimal config without approval_mode, so we use
    // RawConfig::load() + resolve() — the same path the app uses at runtime.
    let content = r#"
[settings]
reply_order = ["claude", "gpt"]

[providers.anthropic]
type = "anthropic"
api_key_env = "ANTHROPIC_API_KEY"

[providers.openai]
type = "openai"
api_key_env = "OPENAI_API_KEY"

[[agents]]
name = "claude"
display_name = "Claude"
provider = "anthropic"
model = "claude-sonnet-4-6"
color = "blue"
enable_thinking = true
enable_web_search = false

[[agents]]
name = "gpt"
display_name = "GPT"
provider = "openai"
model = "gpt-5.4"
color = "green"
enable_thinking = true
enable_web_search = false
"#;

    std::fs::write(&config_path, content).unwrap();

    let raw = RawConfig::load(&config_path).expect("RawConfig::load should succeed");
    let config = raw.resolve();
    config.validate().expect("Config::validate should succeed");

    assert_eq!(config.agents.len(), 2);
    assert_eq!(config.agents[0].name, "claude");
    assert_eq!(config.agents[1].name, "gpt");
    assert_eq!(config.settings.reply_order, vec!["claude", "gpt"]);
    assert!(config.providers.contains_key("anthropic"));
    assert!(config.providers.contains_key("openai"));
}

#[test]
fn batch_generated_config_loads_and_validates() {
    use krew_config::ProviderType;
    use krew_config::RawConfig;
    use krew_config::writer::{AgentWriteData, ProviderWriteData, add_provider, batch_add_agents};

    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("settings.toml");

    // Add providers first.
    add_provider(
        &config_path,
        &ProviderWriteData {
            name: "anthropic".into(),
            provider_type: ProviderType::Anthropic,
            api_key: None,
            api_key_env: Some("ANTHROPIC_API_KEY".into()),
            base_url: None,
            vertex_project: None,
            vertex_location: None,
            extra_headers: None,
        },
    )
    .unwrap();

    // Batch add agents (simulating smart preset).
    let agents = vec![AgentWriteData {
        name: "claude".into(),
        display_name: "Claude".into(),
        provider: "anthropic".into(),
        model: "claude-sonnet-4-6".into(),
        color: "blue".into(),
        enable_thinking: true,
        enable_web_search: false,
        tools: true,
        api_type: None,
        system_prompt: None,
    }];
    batch_add_agents(&config_path, &agents).unwrap();

    // Verify the config loads correctly using the runtime path.
    let raw = RawConfig::load(&config_path).expect("RawConfig::load should succeed");
    let config = raw.resolve();
    config.validate().expect("Config::validate should succeed");

    assert_eq!(config.agents.len(), 1);
    assert_eq!(config.agents[0].name, "claude");
    assert!(config.agents[0].enable_thinking);
}

#[test]
fn vertex_anthropic_generated_config_loads_and_validates() {
    use krew_config::ProviderType;
    use krew_config::RawConfig;
    use krew_config::writer::{AgentWriteData, ProviderWriteData, add_provider, batch_add_agents};

    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("settings.toml");

    add_provider(
        &config_path,
        &ProviderWriteData {
            name: "vertex-anthropic".into(),
            provider_type: ProviderType::VertexAnthropic,
            api_key: None,
            api_key_env: Some("VERTEX_ANTHROPIC_API_KEY".into()),
            base_url: Some("https://litellm.example.com/vertex_ai".into()),
            vertex_project: Some("my-project".into()),
            vertex_location: Some("global".into()),
            extra_headers: None,
        },
    )
    .unwrap();

    let agents = vec![AgentWriteData {
        name: "claude".into(),
        display_name: "Claude".into(),
        provider: "vertex-anthropic".into(),
        model: "claude-opus-4-7".into(),
        color: "blue".into(),
        enable_thinking: true,
        enable_web_search: true,
        tools: true,
        api_type: None,
        system_prompt: None,
    }];
    batch_add_agents(&config_path, &agents).unwrap();

    let raw = RawConfig::load(&config_path).expect("RawConfig::load should succeed");
    let config = raw.resolve();
    config.validate().expect("Config::validate should succeed");

    let provider = &config.providers["vertex-anthropic"];
    assert_eq!(provider.provider_type, ProviderType::VertexAnthropic);
    assert_eq!(provider.vertex_project.as_deref(), Some("my-project"));
    assert_eq!(provider.vertex_location.as_deref(), Some("global"));
    assert!(config.agents[0].enable_web_search);
}
