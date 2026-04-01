//! Tests for `krew config help` manual output.
//!
//! Validates that the hardcoded manual accurately reflects the configuration
//! model by checking for all field names, section headings, and key default
//! values bound to their respective fields.

use std::process::Command;

fn krew_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_krew"))
}

fn help_output() -> String {
    let output = krew_bin()
        .args(["config", "help"])
        .output()
        .expect("failed to run krew config help");
    assert!(output.status.success(), "krew config help failed");
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Assert that `expected_default` appears within 200 chars after `field_name`
/// in the manual output. This binds default values to their fields, preventing
/// drift where a value exists but is attached to the wrong field.
fn assert_field_default(output: &str, field_name: &str, expected_default: &str) {
    let pos = output
        .find(field_name)
        .unwrap_or_else(|| panic!("field '{field_name}' not found in manual output"));
    // Use char-safe slicing to avoid splitting multi-byte chars (e.g. box-drawing).
    let window: String = output[pos..].chars().take(300).collect();
    assert!(
        window.contains(expected_default),
        "field '{field_name}': expected default '{expected_default}' within 300 chars, got:\n{window}"
    );
}

#[test]
fn help_contains_section_headings() {
    let out = help_output();
    for heading in [
        "FILE LOCATIONS",
        "MERGE RULES",
        "CONFIGURATION REFERENCE",
        "EXAMPLE CONFIGURATIONS",
        "CLI COMMANDS",
    ] {
        assert!(out.contains(heading), "missing section heading: {heading}");
    }
}

#[test]
fn help_contains_settings_fields() {
    let out = help_output();
    for field in [
        "approval_mode",
        "reply_order",
        "auto_compact_threshold",
        "compact_keep_rounds",
        "input_history_limit",
        "paste_burst_detection",
        "worker_threads",
        "other_agent_role",
        "allow_rules",
        "deny_rules",
        "ask_rules",
        "agent_to_agent_routing",
        "agent_to_agent_max_rounds",
        "language",
    ] {
        assert!(out.contains(field), "missing [settings] field: {field}");
    }
}

#[test]
fn help_field_defaults_are_bound_to_correct_fields() {
    let out = help_output();

    // [settings] defaults
    assert_field_default(&out, "approval_mode", r#"Default: "suggest""#);
    assert_field_default(&out, "compact_keep_rounds", "Default: 3");
    assert_field_default(&out, "input_history_limit", "Default: 1000");
    assert_field_default(&out, "paste_burst_detection", "Default: true");
    assert_field_default(&out, "worker_threads", "Default: 4");
    assert_field_default(&out, "other_agent_role", r#"Default: "user""#);
    assert_field_default(&out, "agent_to_agent_routing", r#"Default: "immediate""#);
    assert_field_default(&out, "agent_to_agent_max_rounds", "Default: 10");

    // [settings.retry] defaults
    assert_field_default(&out, "max_retries_rate_limit", "Default: 3");
    assert_field_default(&out, "max_retries_server_error", "Default: 2");
    assert_field_default(&out, "backoff_base_secs", "Default: 2.0");
    assert_field_default(&out, "backoff_multiplier", "Default: 3.0");
    assert_field_default(&out, "server_error_interval_secs", "Default: 2.0");
    assert_field_default(&out, "request_timeout_secs", "Default: 60");

    // [[agents]] defaults
    assert_field_default(&out, "  tools ", "Default: true");
    assert_field_default(&out, "  enable_web_search", "Default: false");
    assert_field_default(&out, "  enable_thinking ", "Default: false");

    // [skills] defaults
    assert_field_default(&out, "  enabled ", "Default: true");

    // [[mcp_servers]] trust default
    assert_field_default(&out, "  trust ", r#"Default: "confirm""#);
}

#[test]
fn help_contains_agents_fields() {
    let out = help_output();
    for field in [
        "display_name",
        "api_type",
        "system_prompt",
        "enable_thinking",
        "thinking_effort",
        "temperature",
        "top_p",
        "top_k",
        "max_tokens",
        "frequency_penalty",
        "presence_penalty",
        "stop_sequences",
    ] {
        assert!(
            out.contains(field),
            "missing [[agents]] / sampling field: {field}"
        );
    }
}

#[test]
fn help_contains_providers_fields() {
    let out = help_output();
    for field in [
        "api_key_env",
        "base_url",
        "vertex_project",
        "vertex_location",
        "extra_headers",
    ] {
        assert!(out.contains(field), "missing [providers] field: {field}");
    }
}

#[test]
fn help_contains_mcp_servers_fields() {
    let out = help_output();
    for field in ["command", "args", "url", "headers", "trust"] {
        assert!(
            out.contains(field),
            "missing [[mcp_servers]] field: {field}"
        );
    }
}

#[test]
fn help_contains_skills_and_retry_fields() {
    let out = help_output();

    // skills fields
    assert!(
        out.contains("extra_paths"),
        "missing [skills] field: extra_paths"
    );

    // retry fields
    for field in [
        "max_retries_rate_limit",
        "max_retries_server_error",
        "backoff_base_secs",
        "backoff_multiplier",
        "server_error_interval_secs",
        "request_timeout_secs",
    ] {
        assert!(
            out.contains(field),
            "missing [settings.retry] field: {field}"
        );
    }
}

#[test]
fn help_color_list_matches_parse_color() {
    let out = help_output();
    // All supported colors from parse_color() must appear.
    for color in [
        "red",
        "green",
        "yellow",
        "blue",
        "magenta",
        "cyan",
        "white",
        "gray",
        "dark_gray",
    ] {
        assert!(
            out.contains(color),
            "missing supported color in manual: {color}"
        );
    }
    // bright_* colors are NOT supported — must NOT appear as valid values.
    assert!(
        !out.contains("bright_red"),
        "manual lists unsupported color bright_red"
    );
}
