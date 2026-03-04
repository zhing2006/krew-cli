use krew_config::ApprovalMode;
use krew_tools::ToolRegistry;

use crate::event::ApprovalCache;

/// Result of checking whether a tool call needs user approval.
pub(super) enum ToolApproval {
    /// Tool can be executed without asking the user.
    Auto,
    /// Tool requires user approval before execution.
    NeedsApproval {
        /// Whether the "Approve for Session" option should be shown.
        allow_session_approval: bool,
    },
}

/// Check whether a tool call needs user approval, considering:
/// - The tool's intrinsic approval requirement
/// - The global approval mode
/// - For shell tools: the extracted command prefixes, allowlist, and cache
pub(super) async fn check_tool_approval(
    tool_name: &str,
    arguments: &str,
    tools: &ToolRegistry,
    mode: ApprovalMode,
    cache: &ApprovalCache,
    shell_allow_commands: &[String],
) -> ToolApproval {
    // Readonly tools never need approval.
    if !tools.requires_approval(tool_name) {
        return ToolApproval::Auto;
    }

    // FullAuto mode skips all approval.
    if mode == ApprovalMode::FullAuto {
        return ToolApproval::Auto;
    }

    // AutoEdit mode auto-approves write tools (only shell needs approval).
    if mode == ApprovalMode::AutoEdit && tool_name != "shell" {
        return ToolApproval::Auto;
    }

    // For non-shell tools: simple tool-name-based approval.
    if tool_name != "shell" {
        if cache.is_approved(tool_name).await {
            return ToolApproval::Auto;
        }
        return ToolApproval::NeedsApproval {
            allow_session_approval: true,
        };
    }

    // Shell tool: command-level approval.
    let command = extract_shell_command(arguments);
    let Some(command) = command else {
        // Cannot parse arguments — require approval, no session option.
        return ToolApproval::NeedsApproval {
            allow_session_approval: false,
        };
    };

    let prefixes = krew_tools::builtin::extract_command_prefixes(&command);
    let Some(prefixes) = prefixes else {
        // Complex command — require approval, no session option.
        return ToolApproval::NeedsApproval {
            allow_session_approval: false,
        };
    };

    // Check all prefixes against allowlist and cache.
    let mut all_approved = true;
    for prefix in &prefixes {
        // Check allowlist.
        let in_allowlist = shell_allow_commands
            .iter()
            .any(|entry| krew_tools::builtin::matches_allowlist_entry(prefix, entry));
        if in_allowlist {
            continue;
        }
        // Check session cache (key: "shell:<prefix>").
        let cache_key = format!("shell:{prefix}");
        if !cache.is_approved(&cache_key).await {
            all_approved = false;
            break;
        }
    }

    if all_approved {
        ToolApproval::Auto
    } else {
        ToolApproval::NeedsApproval {
            allow_session_approval: true,
        }
    }
}

/// Cache session approval for a tool call.
///
/// For shell tools, caches each extracted command prefix separately
/// (e.g. `shell:cargo build`). For other tools, caches by tool name.
pub(super) async fn cache_session_approval(
    tool_name: &str,
    arguments: &str,
    cache: &ApprovalCache,
) {
    if tool_name == "shell"
        && let Some(command) = extract_shell_command(arguments)
        && let Some(prefixes) = krew_tools::builtin::extract_command_prefixes(&command)
    {
        for prefix in prefixes {
            let key = format!("shell:{prefix}");
            cache.approve_for_session(key).await;
        }
        return;
    }
    // Non-shell tools or shell parse failure: cache by tool name.
    cache.approve_for_session(tool_name.to_string()).await;
}

/// Extract the shell command string from a tool call's JSON arguments.
fn extract_shell_command(arguments: &str) -> Option<String> {
    let args: serde_json::Value = serde_json::from_str(arguments).ok()?;
    args.get("command")?.as_str().map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_registry() -> ToolRegistry {
        krew_tools::builtin::create_full_registry(PathBuf::from("/tmp"))
    }

    /// Helper to check approval result.
    async fn is_auto(
        tool_name: &str,
        arguments: &str,
        registry: &ToolRegistry,
        mode: ApprovalMode,
        cache: &ApprovalCache,
        allow_cmds: &[String],
    ) -> bool {
        matches!(
            check_tool_approval(tool_name, arguments, registry, mode, cache, allow_cmds).await,
            ToolApproval::Auto
        )
    }

    #[tokio::test]
    async fn suggest_mode_readonly_auto() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow = vec![];
        assert!(
            is_auto(
                "read_file",
                "{}",
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
        assert!(
            is_auto(
                "glob",
                "{}",
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
        assert!(
            is_auto(
                "grep",
                "{}",
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
    }

    #[tokio::test]
    async fn suggest_mode_write_needs_approval() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow = vec![];
        assert!(
            !is_auto(
                "write_file",
                "{}",
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
        assert!(
            !is_auto(
                "edit_file",
                "{}",
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
        // Shell with non-allowlisted command.
        let shell_args = r#"{"command":"rm -rf /"}"#;
        assert!(
            !is_auto(
                "shell",
                shell_args,
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
    }

    #[tokio::test]
    async fn auto_edit_mode_write_auto() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow = vec![];
        assert!(
            is_auto(
                "write_file",
                "{}",
                &registry,
                ApprovalMode::AutoEdit,
                &cache,
                &allow
            )
            .await
        );
        assert!(
            is_auto(
                "edit_file",
                "{}",
                &registry,
                ApprovalMode::AutoEdit,
                &cache,
                &allow
            )
            .await
        );
    }

    #[tokio::test]
    async fn auto_edit_mode_shell_needs_approval() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow = vec![];
        let shell_args = r#"{"command":"rm -rf /"}"#;
        assert!(
            !is_auto(
                "shell",
                shell_args,
                &registry,
                ApprovalMode::AutoEdit,
                &cache,
                &allow
            )
            .await
        );
    }

    #[tokio::test]
    async fn full_auto_mode_all_auto() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow = vec![];
        assert!(
            is_auto(
                "read_file",
                "{}",
                &registry,
                ApprovalMode::FullAuto,
                &cache,
                &allow
            )
            .await
        );
        assert!(
            is_auto(
                "write_file",
                "{}",
                &registry,
                ApprovalMode::FullAuto,
                &cache,
                &allow
            )
            .await
        );
        assert!(
            is_auto(
                "edit_file",
                "{}",
                &registry,
                ApprovalMode::FullAuto,
                &cache,
                &allow
            )
            .await
        );
        let shell_args = r#"{"command":"rm -rf /"}"#;
        assert!(
            is_auto(
                "shell",
                shell_args,
                &registry,
                ApprovalMode::FullAuto,
                &cache,
                &allow
            )
            .await
        );
    }

    #[tokio::test]
    async fn unknown_tool_no_approval() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow = vec![];
        assert!(
            is_auto(
                "unknown_tool",
                "{}",
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
    }

    #[tokio::test]
    async fn shell_allowlist_auto_approves() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow = vec!["ls".to_string(), "cargo build".to_string()];
        // ls is in allowlist.
        let args = r#"{"command":"ls -la"}"#;
        assert!(
            is_auto(
                "shell",
                args,
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
        // cargo build is in allowlist.
        let args = r#"{"command":"cargo build --release"}"#;
        assert!(
            is_auto(
                "shell",
                args,
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
        // cargo test is NOT in allowlist (only cargo build is).
        let args = r#"{"command":"cargo test"}"#;
        assert!(
            !is_auto(
                "shell",
                args,
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
    }

    #[tokio::test]
    async fn shell_session_cache_by_prefix() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow = vec![];
        // Initially needs approval.
        let args = r#"{"command":"cargo build --release"}"#;
        assert!(
            !is_auto(
                "shell",
                args,
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
        // Cache "cargo build" for session.
        cache_session_approval("shell", args, &cache).await;
        // Now auto-approved.
        assert!(
            is_auto(
                "shell",
                args,
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
        // Same prefix with different flags also auto-approved.
        let args2 = r#"{"command":"cargo build -p krew-core"}"#;
        assert!(
            is_auto(
                "shell",
                args2,
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
        // Different subcommand still needs approval.
        let args3 = r#"{"command":"cargo test"}"#;
        assert!(
            !is_auto(
                "shell",
                args3,
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
    }

    #[tokio::test]
    async fn shell_complex_command_no_session_option() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow = vec![];
        // Complex command with command substitution.
        let args = r#"{"command":"echo $(whoami)"}"#;
        let result = check_tool_approval(
            "shell",
            args,
            &registry,
            ApprovalMode::Suggest,
            &cache,
            &allow,
        )
        .await;
        match result {
            ToolApproval::NeedsApproval {
                allow_session_approval,
            } => {
                assert!(!allow_session_approval);
            }
            ToolApproval::Auto => panic!("expected NeedsApproval"),
        }
    }
}
