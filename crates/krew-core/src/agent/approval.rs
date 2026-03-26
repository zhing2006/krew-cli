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
    fetch_allow_domains: &[String],
) -> ToolApproval {
    // Readonly tools never need approval.
    if !tools.requires_approval(tool_name) {
        return ToolApproval::Auto;
    }

    // FullAuto mode skips all approval.
    if mode == ApprovalMode::FullAuto {
        return ToolApproval::Auto;
    }

    // AutoEdit mode auto-approves write tools (only shell/fetch_url need approval).
    if mode == ApprovalMode::AutoEdit && tool_name != "shell" && tool_name != "fetch_url" {
        return ToolApproval::Auto;
    }

    // fetch_url: check domain allowlist.
    if tool_name == "fetch_url" {
        if let Ok(args) = serde_json::from_str::<serde_json::Value>(arguments)
            && krew_tools::builtin::fetch_url::is_fetch_domain_allowed(&args, fetch_allow_domains)
        {
            return ToolApproval::Auto;
        }
        // Per-host session cache for fetch_url.
        if let Some(host) = extract_fetch_host(arguments) {
            let cache_key = format!("fetch_url:{host}");
            if cache.is_approved(&cache_key).await {
                return ToolApproval::Auto;
            }
        }
        return ToolApproval::NeedsApproval {
            allow_session_approval: true,
        };
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
    // fetch_url: cache by host so different hosts still require approval.
    if tool_name == "fetch_url" {
        if let Some(host) = extract_fetch_host(arguments) {
            let key = format!("fetch_url:{host}");
            cache.approve_for_session(key).await;
        } else {
            cache.approve_for_session(tool_name.to_string()).await;
        }
        return;
    }
    // Non-shell, non-fetch tools or shell parse failure: cache by tool name.
    cache.approve_for_session(tool_name.to_string()).await;
}

/// Extract the shell command string from a tool call's JSON arguments.
fn extract_shell_command(arguments: &str) -> Option<String> {
    let args: serde_json::Value = serde_json::from_str(arguments).ok()?;
    args.get("command")?.as_str().map(|s| s.to_string())
}

/// Extract the host from a fetch_url tool call's JSON arguments.
///
/// Delegates to `fetch_url::extract_url_host` which handles URL
/// normalization (missing scheme, http→https upgrade) consistently
/// with the tool's own logic.
fn extract_fetch_host(arguments: &str) -> Option<String> {
    let args: serde_json::Value = serde_json::from_str(arguments).ok()?;
    let url_str = args.get("url")?.as_str()?;
    krew_tools::builtin::fetch_url::extract_url_host(url_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_registry() -> ToolRegistry {
        krew_tools::builtin::create_full_registry(
            PathBuf::from("/tmp"),
            true,
            std::collections::HashMap::new(),
        )
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
            check_tool_approval(tool_name, arguments, registry, mode, cache, allow_cmds, &[]).await,
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
    async fn fetch_url_needs_approval_by_default() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow_cmds = vec![];
        let args = r#"{"url":"https://evil.com/page"}"#;
        let result = check_tool_approval(
            "fetch_url",
            args,
            &registry,
            ApprovalMode::Suggest,
            &cache,
            &allow_cmds,
            &[],
        )
        .await;
        assert!(matches!(result, ToolApproval::NeedsApproval { .. }));
    }

    #[tokio::test]
    async fn fetch_url_allowed_domain_auto() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow_cmds = vec![];
        let fetch_domains = vec!["docs.rs".to_string()];
        let args = r#"{"url":"https://docs.rs/htmd/latest"}"#;
        let result = check_tool_approval(
            "fetch_url",
            args,
            &registry,
            ApprovalMode::Suggest,
            &cache,
            &allow_cmds,
            &fetch_domains,
        )
        .await;
        assert!(matches!(result, ToolApproval::Auto));
    }

    #[tokio::test]
    async fn fetch_url_subdomain_auto() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow_cmds = vec![];
        let fetch_domains = vec!["github.com".to_string()];
        let args = r#"{"url":"https://docs.github.com/en/repos"}"#;
        let result = check_tool_approval(
            "fetch_url",
            args,
            &registry,
            ApprovalMode::Suggest,
            &cache,
            &allow_cmds,
            &fetch_domains,
        )
        .await;
        assert!(matches!(result, ToolApproval::Auto));
    }

    #[tokio::test]
    async fn fetch_url_full_auto_mode() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow_cmds = vec![];
        let args = r#"{"url":"https://evil.com/page"}"#;
        assert!(matches!(
            check_tool_approval(
                "fetch_url",
                args,
                &registry,
                ApprovalMode::FullAuto,
                &cache,
                &allow_cmds,
                &[],
            )
            .await,
            ToolApproval::Auto
        ));
    }

    #[tokio::test]
    async fn fetch_url_session_cache_by_host() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow_cmds = vec![];

        // Initially needs approval.
        let args = r#"{"url":"https://docs.rs/htmd/latest"}"#;
        let result = check_tool_approval(
            "fetch_url",
            args,
            &registry,
            ApprovalMode::Suggest,
            &cache,
            &allow_cmds,
            &[],
        )
        .await;
        assert!(matches!(result, ToolApproval::NeedsApproval { .. }));

        // Cache approval for this URL.
        cache_session_approval("fetch_url", args, &cache).await;

        // Same host auto-approved.
        let args2 = r#"{"url":"https://docs.rs/serde/latest"}"#;
        let result = check_tool_approval(
            "fetch_url",
            args2,
            &registry,
            ApprovalMode::Suggest,
            &cache,
            &allow_cmds,
            &[],
        )
        .await;
        assert!(matches!(result, ToolApproval::Auto));

        // Different host still needs approval.
        let args3 = r#"{"url":"https://evil.com/page"}"#;
        let result = check_tool_approval(
            "fetch_url",
            args3,
            &registry,
            ApprovalMode::Suggest,
            &cache,
            &allow_cmds,
            &[],
        )
        .await;
        assert!(matches!(result, ToolApproval::NeedsApproval { .. }));
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
            &[],
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
