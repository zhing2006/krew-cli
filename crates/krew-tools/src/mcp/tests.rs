//! Tests for MCP module.

use krew_config::McpTrust;

use super::*;
use crate::mcp::client::McpToolAnnotations;
use crate::mcp::handler::check_mcp_approval;
use crate::mcp::manager::expand_env;

// ─── Qualified name generation ─────────────────────────────────────

#[test]
fn qualified_name_basic() {
    assert_eq!(
        qualified_name("filesystem", "list_directory"),
        "mcp__filesystem_list_directory"
    );
}

#[test]
fn qualified_name_sanitizes_special_chars() {
    assert_eq!(
        qualified_name("my.server", "list.files"),
        "mcp__my_server_list_files"
    );
}

#[test]
fn qualified_name_preserves_hyphens() {
    assert_eq!(
        qualified_name("my-server", "list-files"),
        "mcp__my-server_list-files"
    );
}

#[test]
fn qualified_name_sanitizes_spaces_and_unicode() {
    assert_eq!(
        qualified_name("my server", "查询文件"),
        "mcp__my_server_____"
    );
}

// ─── Display name generation ───────────────────────────────────────

#[test]
fn display_name_basic() {
    assert_eq!(
        display_name("filesystem", "list_directory"),
        "mcp:filesystem/list_directory"
    );
}

// ─── Display name from qualified ───────────────────────────────────

#[test]
fn display_name_from_qualified_basic() {
    assert_eq!(
        display_name_from_qualified("mcp__filesystem_list_directory"),
        Some("mcp:filesystem/list_directory".to_string())
    );
}

#[test]
fn display_name_from_qualified_not_mcp() {
    assert_eq!(display_name_from_qualified("shell"), None);
    assert_eq!(display_name_from_qualified("read_file"), None);
}

#[test]
fn display_name_from_qualified_no_separator() {
    // "mcp__serveronly" has no second underscore after prefix removal
    // "serveronly" → split_once('_') → None
    assert_eq!(display_name_from_qualified("mcp__serveronly"), None);
}

// ─── is_mcp_tool ───────────────────────────────────────────────────

#[test]
fn is_mcp_tool_detects_mcp() {
    assert!(is_mcp_tool("mcp__filesystem_list"));
    assert!(!is_mcp_tool("shell"));
    assert!(!is_mcp_tool("read_file"));
    assert!(!is_mcp_tool("mcp_single_underscore"));
}

// ─── Sanitize ──────────────────────────────────────────────────────

#[test]
fn sanitize_alphanumeric() {
    assert_eq!(sanitize("abc123"), "abc123");
}

#[test]
fn sanitize_preserves_underscores_and_hyphens() {
    assert_eq!(sanitize("my_tool-name"), "my_tool-name");
}

#[test]
fn sanitize_replaces_dots_and_slashes() {
    assert_eq!(sanitize("my.tool/name"), "my_tool_name");
}

// ─── Environment variable expansion ───────────────────────────────

#[test]
fn expand_env_none_returns_empty() {
    let result = expand_env(&None);
    assert!(result.is_empty());
}

#[test]
fn expand_env_literal_values() {
    let mut map = std::collections::HashMap::new();
    map.insert("KEY".to_string(), "value".to_string());

    let result = expand_env(&Some(map));
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], ("KEY".to_string(), "value".to_string()));
}

#[test]
fn expand_env_resolves_env_var() {
    // SAFETY: This test runs single-threaded and uses a unique env var name.
    unsafe { std::env::set_var("KREW_TEST_TOKEN_1234", "secret123") };

    let mut map = std::collections::HashMap::new();
    map.insert("TOKEN".to_string(), "$KREW_TEST_TOKEN_1234".to_string());

    let result = expand_env(&Some(map));
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], ("TOKEN".to_string(), "secret123".to_string()));

    unsafe { std::env::remove_var("KREW_TEST_TOKEN_1234") };
}

#[test]
fn expand_env_missing_var_becomes_empty() {
    let mut map = std::collections::HashMap::new();
    map.insert("TOKEN".to_string(), "$KREW_NONEXISTENT_VAR_XYZ".to_string());

    let result = expand_env(&Some(map));
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], ("TOKEN".to_string(), String::new()));
}

// ─── MCP approval logic ───────────────────────────────────────────

#[test]
fn approval_trust_auto_always_false() {
    assert!(!check_mcp_approval(McpTrust::Auto, None));
}

#[test]
fn approval_trust_auto_ignores_destructive_hint() {
    let ann = McpToolAnnotations {
        destructive_hint: Some(true),
        read_only_hint: None,
        open_world_hint: None,
        idempotent_hint: None,
    };
    assert!(!check_mcp_approval(McpTrust::Auto, Some(&ann)));
}

#[test]
fn approval_trust_confirm_no_annotations_requires() {
    assert!(check_mcp_approval(McpTrust::Confirm, None));
}

#[test]
fn approval_trust_confirm_read_only_auto() {
    let ann = McpToolAnnotations {
        destructive_hint: None,
        read_only_hint: Some(true),
        open_world_hint: None,
        idempotent_hint: None,
    };
    assert!(!check_mcp_approval(McpTrust::Confirm, Some(&ann)));
}

#[test]
fn approval_trust_confirm_destructive_requires() {
    let ann = McpToolAnnotations {
        destructive_hint: Some(true),
        read_only_hint: None,
        open_world_hint: None,
        idempotent_hint: None,
    };
    assert!(check_mcp_approval(McpTrust::Confirm, Some(&ann)));
}

#[test]
fn approval_trust_confirm_read_only_false_requires() {
    let ann = McpToolAnnotations {
        destructive_hint: None,
        read_only_hint: Some(false),
        open_world_hint: None,
        idempotent_hint: None,
    };
    assert!(check_mcp_approval(McpTrust::Confirm, Some(&ann)));
}

#[test]
fn approval_trust_confirm_all_none_requires() {
    let ann = McpToolAnnotations {
        destructive_hint: None,
        read_only_hint: None,
        open_world_hint: None,
        idempotent_hint: None,
    };
    assert!(check_mcp_approval(McpTrust::Confirm, Some(&ann)));
}

#[test]
fn approval_trust_confirm_read_only_overrides_destructive() {
    // When both are set, read_only_hint takes priority (checked first).
    let ann = McpToolAnnotations {
        destructive_hint: Some(true),
        read_only_hint: Some(true),
        open_world_hint: None,
        idempotent_hint: None,
    };
    assert!(!check_mcp_approval(McpTrust::Confirm, Some(&ann)));
}

// ─── ToolSpec creation ─────────────────────────────────────────────

#[test]
fn tool_spec_description_includes_display_name() {
    let qname = qualified_name("github", "create_issue");
    let dname = display_name("github", "create_issue");

    let spec = crate::ToolSpec {
        name: qname,
        description: format!("[{dname}] Create a GitHub issue"),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "title": { "type": "string" }
            }
        }),
    };

    assert_eq!(spec.name, "mcp__github_create_issue");
    assert!(spec.description.starts_with("[mcp:github/create_issue]"));
}
