use std::io::Write;
use tempfile::NamedTempFile;

use krew_config::{
    AgentConfig, ApprovalMode, Config, ConfigError, McpServerConfig, ProviderConfig, ThinkingEffort,
};

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

#[test]
fn load_missing_agents_is_parse_error() {
    // Config::load() should reject a file with no [[agents]] section.
    let toml = r#"
[settings]
approval_mode = "suggest"
reply_order = []

[providers.openai]
type = "openai"
api_key_env = "KEY"
"#;
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(toml.as_bytes()).unwrap();
    let result = Config::load(f.path());
    assert!(
        result.is_err(),
        "missing [[agents]] should fail at parse time"
    );
}

#[test]
fn load_missing_settings_is_parse_error() {
    // Config::load() should reject a file with no [settings] section.
    let toml = r#"
[[agents]]
name = "a"
display_name = "A"
provider = "builtin"
model = "echo"
color = "red"
"#;
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(toml.as_bytes()).unwrap();
    let result = Config::load(f.path());
    assert!(
        result.is_err(),
        "missing [settings] should fail at parse time"
    );
}

// ── Config::default() ───────────────────────────────────────────────────

#[test]
fn default_config_has_no_agents() {
    let config = Config::default();
    assert!(config.agents.is_empty());
    assert!(config.settings.reply_order.is_empty());
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
fn validate_reserved_agent_name_all() {
    let toml = r#"
[settings]
approval_mode = "suggest"
reply_order = ["all"]

[[agents]]
name = "all"
display_name = "All Agent"
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
        msg.contains("reserved"),
        "error should mention reserved: {msg}"
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

// ── ThinkingEffort deserialization ──────────────────────────────────────

#[derive(serde::Deserialize)]
struct ThinkingEffortWrapper {
    val: ThinkingEffort,
}

#[test]
fn thinking_effort_deserialize_low() {
    let w: ThinkingEffortWrapper = toml::from_str("val = \"low\"").unwrap();
    assert_eq!(w.val, ThinkingEffort::Low);
}

#[test]
fn thinking_effort_deserialize_medium() {
    let w: ThinkingEffortWrapper = toml::from_str("val = \"medium\"").unwrap();
    assert_eq!(w.val, ThinkingEffort::Medium);
}

#[test]
fn thinking_effort_deserialize_high() {
    let w: ThinkingEffortWrapper = toml::from_str("val = \"high\"").unwrap();
    assert_eq!(w.val, ThinkingEffort::High);
}

#[test]
fn thinking_effort_deserialize_invalid() {
    let result: Result<ThinkingEffortWrapper, _> = toml::from_str("val = \"extreme\"");
    assert!(result.is_err());
}

// ── AgentConfig deserialization ─────────────────────────────────────────

#[test]
fn agent_config_enable_thinking_default_false() {
    let toml_str = r#"
        name = "test"
        display_name = "Test"
        provider = "openai"
        model = "gpt-4"
        color = "blue"
    "#;
    let agent: AgentConfig = toml::from_str(toml_str).unwrap();
    assert!(!agent.enable_thinking);
}

#[test]
fn agent_config_enable_thinking_with_effort() {
    let toml_str = r#"
        name = "test"
        display_name = "Test"
        provider = "openai"
        model = "gpt-4"
        color = "blue"
        enable_thinking = true
        thinking_effort = "high"
    "#;
    let agent: AgentConfig = toml::from_str(toml_str).unwrap();
    assert!(agent.enable_thinking);
    assert_eq!(agent.thinking_effort, Some(ThinkingEffort::High));
}

#[test]
fn agent_config_enable_thinking_without_effort() {
    let toml_str = r#"
        name = "test"
        display_name = "Test"
        provider = "openai"
        model = "gpt-4"
        color = "blue"
        enable_thinking = true
    "#;
    let agent: AgentConfig = toml::from_str(toml_str).unwrap();
    assert!(agent.enable_thinking);
    assert!(agent.thinking_effort.is_none());
}

// ── ProviderConfig deserialization ──────────────────────────────────────

#[test]
fn provider_config_vertex_fields() {
    let toml_str = r#"
        type = "google"
        api_key_env = "GOOGLE_API_KEY"
        vertex_project = "my-proj"
        vertex_location = "us-central1"
    "#;
    let provider: ProviderConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(provider.vertex_project.as_deref(), Some("my-proj"));
    assert_eq!(provider.vertex_location.as_deref(), Some("us-central1"));
}

#[test]
fn provider_config_vertex_fields_missing() {
    let toml_str = r#"
        type = "google"
        api_key_env = "GOOGLE_API_KEY"
    "#;
    let provider: ProviderConfig = toml::from_str(toml_str).unwrap();
    assert!(provider.vertex_project.is_none());
    assert!(provider.vertex_location.is_none());
}

#[test]
fn provider_config_extra_headers() {
    let toml_str = r#"
        type = "google"
        vertex_project = "my-proj"
        vertex_location = "global"
        extra_headers = { "X-Vertex-AI-LLM-Request-Type" = "shared", "X-Vertex-AI-LLM-Shared-Request-Type" = "priority" }
    "#;
    let provider: ProviderConfig = toml::from_str(toml_str).unwrap();
    let headers = provider.extra_headers.unwrap();
    assert_eq!(headers.len(), 2);
    assert_eq!(
        headers.get("X-Vertex-AI-LLM-Request-Type").unwrap(),
        "shared"
    );
    assert_eq!(
        headers.get("X-Vertex-AI-LLM-Shared-Request-Type").unwrap(),
        "priority"
    );
}

#[test]
fn provider_config_extra_headers_missing() {
    let toml_str = r#"
        type = "google"
        api_key_env = "GOOGLE_API_KEY"
    "#;
    let provider: ProviderConfig = toml::from_str(toml_str).unwrap();
    assert!(provider.extra_headers.is_none());
}

// ── Full config E2E ─────────────────────────────────────────────────────

#[test]
fn full_config_e2e_with_new_fields() {
    let toml_str = r#"
        [settings]
        approval_mode = "suggest"
        reply_order = ["agent1"]

        [[agents]]
        name = "agent1"
        display_name = "Agent 1"
        provider = "anthropic"
        model = "claude-opus-4-6"
        color = "green"
        enable_thinking = true
        thinking_effort = "medium"

        [providers.anthropic]
        type = "anthropic"
        api_key_env = "ANTHROPIC_API_KEY"

        [providers.google]
        type = "google"
        api_key_env = "GOOGLE_API_KEY"
        vertex_project = "my-proj"
        vertex_location = "us-central1"
    "#;
    let config: Config = toml::from_str(toml_str).unwrap();
    let agent = &config.agents[0];
    assert!(agent.enable_thinking);
    assert_eq!(agent.thinking_effort, Some(ThinkingEffort::Medium));

    let google = &config.providers["google"];
    assert_eq!(google.vertex_project.as_deref(), Some("my-proj"));
    assert_eq!(google.vertex_location.as_deref(), Some("us-central1"));
}

// ── McpServerConfig deserialization ─────────────────────────────────────

#[test]
fn mcp_server_config_stdio() {
    let toml_str = r#"
        name = "filesystem"
        command = "npx"
        args = ["-y", "@modelcontextprotocol/server-filesystem"]
    "#;
    let config: McpServerConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.name, "filesystem");
    assert_eq!(config.command.as_deref(), Some("npx"));
    assert_eq!(
        config.args,
        vec!["-y", "@modelcontextprotocol/server-filesystem"]
    );
    assert!(!config.is_http());
    assert!(config.url.is_none());
    assert!(config.headers.is_none());
}

#[test]
fn mcp_server_config_http() {
    let toml_str = r#"
        name = "firecrawl"
        url = "https://mcp.firecrawl.dev/v2/mcp"

        [headers]
        Authorization = "Bearer fc-abc123"
    "#;
    let config: McpServerConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.name, "firecrawl");
    assert!(config.is_http());
    assert_eq!(
        config.url.as_deref(),
        Some("https://mcp.firecrawl.dev/v2/mcp")
    );
    let headers = config.headers.as_ref().unwrap();
    assert_eq!(headers.get("Authorization").unwrap(), "Bearer fc-abc123");
    assert!(config.command.is_none());
}

#[test]
fn mcp_server_config_http_no_headers() {
    let toml_str = r#"
        name = "local"
        url = "http://localhost:8080/mcp"
    "#;
    let config: McpServerConfig = toml::from_str(toml_str).unwrap();
    assert!(config.is_http());
    assert!(config.headers.is_none());
}

#[test]
fn mcp_server_config_in_full_config() {
    let toml_str = r#"
        [settings]
        approval_mode = "suggest"
        reply_order = ["agent1"]

        [[agents]]
        name = "agent1"
        display_name = "Agent 1"
        provider = "openai"
        model = "gpt-5"
        color = "green"

        [providers.openai]
        type = "openai"
        api_key_env = "OPENAI_API_KEY"

        [[mcp_servers]]
        name = "stdio-server"
        command = "node"
        args = ["server.js"]

        [[mcp_servers]]
        name = "http-server"
        url = "https://example.com/mcp"
        [mcp_servers.headers]
        Authorization = "Bearer token123"
        X-Custom = "value"
    "#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(config.mcp_servers.len(), 2);

    let stdio = &config.mcp_servers[0];
    assert_eq!(stdio.name, "stdio-server");
    assert!(!stdio.is_http());
    assert_eq!(stdio.command.as_deref(), Some("node"));

    let http = &config.mcp_servers[1];
    assert_eq!(http.name, "http-server");
    assert!(http.is_http());
    assert_eq!(http.url.as_deref(), Some("https://example.com/mcp"));
    let headers = http.headers.as_ref().unwrap();
    assert_eq!(headers.len(), 2);
    assert_eq!(headers.get("Authorization").unwrap(), "Bearer token123");
}
