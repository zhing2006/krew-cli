use krew_config::{ApprovalMode, PermissionRule};
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
        /// Optional reason from an ask rule or bypass immunity.
        reason: Option<String>,
    },
    /// Tool is denied by a rule — return error to LLM without asking user.
    Denied {
        /// Reason for denial.
        reason: String,
    },
}

// ── Hardcoded protection ────────────────────────────────────────────

/// Directories whose contents are always protected from silent modification.
const DANGEROUS_DIRECTORIES: &[&str] = &[".git", ".krew", ".vscode", ".idea", ".claude"];

/// Files that are always protected from silent modification.
const DANGEROUS_FILES: &[&str] = &[
    ".gitconfig",
    ".gitmodules",
    ".bashrc",
    ".bash_profile",
    ".zshrc",
    ".zprofile",
    ".profile",
    ".env",
];

/// Check if a file path targets a protected (dangerous) location.
///
/// - For `DANGEROUS_DIRECTORIES`: any path segment matching a directory name.
/// - For `DANGEROUS_FILES`: the filename component matching a file name.
/// - Case-insensitive (for Windows compatibility).
/// - Windows backslashes are normalized to forward slashes.
pub fn is_dangerous_path(file_path: &str) -> bool {
    let normalized = file_path.replace('\\', "/");
    let lower = normalized.to_lowercase();

    // Check directory segments.
    for segment in lower.split('/') {
        if segment.is_empty() {
            continue;
        }
        for dir in DANGEROUS_DIRECTORIES {
            if segment == *dir {
                return true;
            }
        }
    }

    // Check filename.
    if let Some(filename) = lower.rsplit('/').next() {
        for file in DANGEROUS_FILES {
            if filename == *file {
                return true;
            }
        }
    }

    false
}

/// Built-in shell deny patterns that protect critical paths.
///
/// These are checked against each command segment extracted by shell_parse.
/// Returns a deny reason if a dangerous pattern is detected.
pub fn is_dangerous_shell_command(command: &str) -> Option<String> {
    // First check if the raw command contains redirection to protected paths.
    // Redirection causes shell_parse to return None (complex construct),
    // so we check the raw string directly.
    let lower = command.to_lowercase();
    for dir in DANGEROUS_DIRECTORIES {
        if lower.contains(&format!("> {dir}")) || lower.contains(&format!(">{dir}")) {
            return Some(format!(
                "Redirecting output to protected path '{dir}' is not allowed"
            ));
        }
    }
    for file in DANGEROUS_FILES {
        if lower.contains(&format!("> {file}")) || lower.contains(&format!(">{file}")) {
            return Some(format!(
                "Redirecting output to protected file '{file}' is not allowed"
            ));
        }
    }

    // Try to parse command segments.
    let segments = krew_tools::builtin::extract_command_prefixes(command);
    if let Some(prefixes) = &segments {
        for prefix in prefixes {
            let lower_prefix = prefix.to_lowercase();
            // Check rm targeting protected paths.
            if lower_prefix == "rm" || lower_prefix.starts_with("rm ") {
                let args_lower = command.to_lowercase();
                for dir in DANGEROUS_DIRECTORIES {
                    if args_lower.contains(dir) {
                        return Some(format!("Deleting protected path '{dir}' is not allowed"));
                    }
                }
                for file in DANGEROUS_FILES {
                    if args_lower.contains(file) {
                        return Some(format!("Deleting protected file '{file}' is not allowed"));
                    }
                }
            }
        }
    } else {
        // Complex command: check raw string for rm + protected paths.
        for dir in DANGEROUS_DIRECTORIES {
            if lower.contains("rm") && lower.contains(dir) {
                return Some(format!("Command appears to delete protected path '{dir}'"));
            }
        }
        for file in DANGEROUS_FILES {
            if lower.contains("rm") && lower.contains(file) {
                return Some(format!("Command appears to delete protected file '{file}'"));
            }
        }
    }

    None
}

// ── Pattern matching ────────────────────────────────────────────────

/// Match a shell command against a wildcard pattern.
///
/// `*` matches any character sequence, `\*` matches literal `*`.
/// Trailing ` *` makes the space-and-args optional (e.g. `cargo *` matches `cargo`).
pub fn match_shell_wildcard(pattern: &str, command: &str) -> bool {
    let trimmed_pattern = pattern.trim();
    let trimmed_command = command.trim();

    if trimmed_pattern.is_empty() {
        return trimmed_command.is_empty();
    }

    // Process escape sequences.
    let mut processed = String::new();
    let chars: Vec<char> = trimmed_pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            let next = chars[i + 1];
            if next == '*' {
                processed.push('\x01'); // placeholder for literal *
                i += 2;
                continue;
            } else if next == '\\' {
                processed.push('\x02'); // placeholder for literal \
                i += 2;
                continue;
            }
        }
        processed.push(chars[i]);
        i += 1;
    }

    // Count unescaped wildcards.
    let unescaped_star_count = processed.chars().filter(|&c| c == '*').count();

    // Escape regex special chars (except *).
    let mut escaped = String::new();
    for c in processed.chars() {
        match c {
            '.' | '+' | '?' | '^' | '$' | '{' | '}' | '(' | ')' | '|' | '[' | ']' | '\'' | '"' => {
                escaped.push('\\');
                escaped.push(c);
            }
            _ => escaped.push(c),
        }
    }

    // Convert * to .* for wildcard.
    let with_wildcards = escaped.replace('*', ".*");

    // Convert placeholders back.
    let mut regex_pattern = with_wildcards
        .replace('\x01', "\\*")
        .replace('\x02', "\\\\");

    // Trailing ` *` with single wildcard: make space+args optional.
    if regex_pattern.ends_with(" .*") && unescaped_star_count == 1 {
        regex_pattern = format!("{}( .*)?", &regex_pattern[..regex_pattern.len() - 3]);
    }

    let regex_str = format!("^{regex_pattern}$");
    match regex::Regex::new(&regex_str) {
        Ok(re) => re.is_match(trimmed_command),
        Err(_) => false,
    }
}

/// Normalize a file path for glob matching.
///
/// 1. Windows backslashes → forward slashes
/// 2. Absolute path → relative to cwd
/// 3. Resolve `.` and `..` segments
/// 4. Strip leading `./`
pub fn normalize_file_path(file_path: &str, cwd: &str) -> String {
    let mut path = file_path.replace('\\', "/");

    // Convert absolute path to relative.
    let cwd_normalized = cwd.replace('\\', "/");
    let cwd_prefix = if cwd_normalized.ends_with('/') {
        cwd_normalized.clone()
    } else {
        format!("{cwd_normalized}/")
    };
    if path.starts_with(&cwd_prefix) {
        path = path[cwd_prefix.len()..].to_string();
    } else if path == cwd_normalized {
        path = ".".to_string();
    }

    // Resolve . and .. segments.
    let mut resolved = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => continue,
            ".." => {
                resolved.pop();
            }
            s => resolved.push(s),
        }
    }

    let result = resolved.join("/");
    if result.is_empty() {
        ".".to_string()
    } else {
        result
    }
}

/// Match a file path against a glob pattern.
///
/// Supports:
/// - `*` matches any characters within a single directory
/// - `**` matches any number of directories
/// - Exact filename match (e.g. `.env`)
pub fn match_file_glob(pattern: &str, path: &str) -> bool {
    let pattern = pattern.replace('\\', "/");
    let path = path.replace('\\', "/");

    glob_match_recursive(&pattern, &path)
}

fn glob_match_recursive(pattern: &str, path: &str) -> bool {
    // Split into segments.
    let pat_segments: Vec<&str> = pattern.split('/').collect();
    let path_segments: Vec<&str> = path.split('/').collect();

    glob_match_segments(&pat_segments, &path_segments)
}

fn glob_match_segments(pat: &[&str], path: &[&str]) -> bool {
    if pat.is_empty() {
        return path.is_empty();
    }

    if pat[0] == "**" {
        // ** matches zero or more directory segments.
        // Try matching remaining pattern against every suffix of path.
        for i in 0..=path.len() {
            if glob_match_segments(&pat[1..], &path[i..]) {
                return true;
            }
        }
        return false;
    }

    if path.is_empty() {
        return false;
    }

    // Match current segment with simple wildcard.
    if glob_match_segment(pat[0], path[0]) {
        glob_match_segments(&pat[1..], &path[1..])
    } else {
        false
    }
}

fn glob_match_segment(pattern: &str, segment: &str) -> bool {
    // Simple wildcard matching within a segment.
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == segment;
    }

    // Convert to regex for segment matching.
    let mut regex_str = String::from("^");
    for c in pattern.chars() {
        match c {
            '*' => regex_str.push_str("[^/]*"),
            '.' | '+' | '?' | '^' | '$' | '{' | '}' | '(' | ')' | '|' | '[' | ']' => {
                regex_str.push('\\');
                regex_str.push(c);
            }
            _ => regex_str.push(c),
        }
    }
    regex_str.push('$');
    match regex::Regex::new(&regex_str) {
        Ok(re) => re.is_match(segment),
        Err(_) => false,
    }
}

/// Match a URL host against a domain suffix pattern.
///
/// `github.com` matches `github.com` and `docs.github.com`.
fn match_domain_suffix(pattern: &str, host: &str) -> bool {
    let host = host.to_lowercase();
    let pattern = pattern.to_lowercase();
    host == pattern || host.ends_with(&format!(".{pattern}"))
}

// ── Rule matching ───────────────────────────────────────────────────

/// Extract the file path from a tool call's JSON arguments.
fn extract_file_path(arguments: &str) -> Option<String> {
    let args: serde_json::Value = serde_json::from_str(arguments).ok()?;
    args.get("file_path")
        .or_else(|| args.get("path"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Extract the shell command string from a tool call's JSON arguments.
fn extract_shell_command(arguments: &str) -> Option<String> {
    let args: serde_json::Value = serde_json::from_str(arguments).ok()?;
    args.get("command")?.as_str().map(|s| s.to_string())
}

/// Extract the host from a fetch_url tool call's JSON arguments.
fn extract_fetch_host(arguments: &str) -> Option<String> {
    let args: serde_json::Value = serde_json::from_str(arguments).ok()?;
    let url_str = args.get("url")?.as_str()?;
    krew_tools::builtin::fetch_url::extract_url_host(url_str)
}

/// Check if a shell command matches a rule's pattern, handling compound commands.
///
/// For deny/ask: any segment matching → true.
/// For allow: all segments must match → true.
fn matches_shell_rule(command: &str, pattern: &str, require_all: bool) -> bool {
    let is_complex = krew_tools::builtin::extract_command_prefixes(command).is_none();

    if is_complex {
        if require_all {
            // Complex command, allow doesn't apply.
            false
        } else {
            // Complex command, deny/ask: try whole-string matching.
            match_shell_wildcard(pattern, command)
        }
    } else {
        // Parseable: use the quote-aware splitter to get segments.
        let parts: Vec<String> = krew_tools::builtin::split_shell_operators(command);
        let trimmed: Vec<&str> = parts
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if require_all {
            // Allow: all segments must match.
            trimmed
                .iter()
                .all(|part| match_shell_wildcard(pattern, part))
        } else {
            // Deny/ask: any segment matching.
            trimmed
                .iter()
                .any(|part| match_shell_wildcard(pattern, part))
        }
    }
}

/// Check if a tool call matches a permission rule.
///
/// `require_all` controls compound shell command behavior:
/// - `false` (deny/ask): any segment match → true
/// - `true` (allow): all segments must match → true
pub fn matches_rule(
    tool_name: &str,
    arguments: &str,
    rule: &PermissionRule,
    cwd: &str,
    require_all: bool,
) -> bool {
    if rule.tool != tool_name {
        return false;
    }

    let Some(pattern) = &rule.pattern else {
        // No pattern: match all calls to this tool.
        return true;
    };

    match tool_name {
        "shell" => {
            let Some(command) = extract_shell_command(arguments) else {
                return false;
            };
            matches_shell_rule(&command, pattern, require_all)
        }
        "write_file" | "edit_file" | "read_file" => {
            let Some(file_path) = extract_file_path(arguments) else {
                return false;
            };
            let normalized = normalize_file_path(&file_path, cwd);
            match_file_glob(pattern, &normalized)
        }
        "fetch_url" => {
            let Some(host) = extract_fetch_host(arguments) else {
                return false;
            };
            match_domain_suffix(pattern, &host)
        }
        _ => {
            // Other tools: pattern is ignored, match on tool name only.
            true
        }
    }
}

// ── 8-step approval pipeline ────────────────────────────────────────

/// Context for tool approval checks, grouping permission-related state.
pub(super) struct ApprovalContext<'a> {
    pub tools: &'a ToolRegistry,
    pub mode: ApprovalMode,
    pub cache: &'a ApprovalCache,
    pub allow_rules: &'a [PermissionRule],
    pub deny_rules: &'a [PermissionRule],
    pub ask_rules: &'a [PermissionRule],
    pub cwd: &'a str,
}

/// Check whether a tool call needs user approval using the 8-step pipeline.
///
/// Step 0: User deny rules (highest priority — explicit user intent)
/// Step 1: Bypass immunity (protected paths + built-in shell deny)
/// Step 2: User ask rules (bypass-immune — even FullAuto must confirm)
/// Step 3: Readonly tools → Auto
/// Step 4: FullAuto mode → Auto
/// Step 5: User allow rules
/// Step 6: Session cache
/// Step 7: AutoEdit + write tools → Auto
/// Step 8: Default → NeedsApproval
pub(super) async fn check_tool_approval(
    tool_name: &str,
    arguments: &str,
    ctx: &ApprovalContext<'_>,
) -> ToolApproval {
    let cwd = ctx.cwd;

    // Step 0: User deny rules — highest priority, explicit user intent.
    for rule in ctx.deny_rules {
        if matches_rule(tool_name, arguments, rule, cwd, false) {
            let reason = rule
                .reason
                .clone()
                .unwrap_or_else(|| "Denied by rule.".to_string());
            return ToolApproval::Denied { reason };
        }
    }

    // Step 1: Bypass immunity — protected paths and built-in shell deny.
    if matches!(tool_name, "write_file" | "edit_file" | "read_file")
        && let Some(file_path) = extract_file_path(arguments)
    {
        let normalized = normalize_file_path(&file_path, cwd);
        if is_dangerous_path(&normalized) {
            return ToolApproval::NeedsApproval {
                allow_session_approval: false,
                reason: Some(format!("Protected path: {normalized}")),
            };
        }
    }
    if tool_name == "shell"
        && let Some(command) = extract_shell_command(arguments)
        && let Some(reason) = is_dangerous_shell_command(&command)
    {
        return ToolApproval::Denied { reason };
    }

    // Step 2: User ask rules (bypass-immune — even FullAuto must confirm).
    for rule in ctx.ask_rules {
        if matches_rule(tool_name, arguments, rule, cwd, false) {
            return ToolApproval::NeedsApproval {
                allow_session_approval: true,
                reason: rule.reason.clone(),
            };
        }
    }

    // Step 3: Readonly tools (no deny/ask match) → Auto.
    if !ctx.tools.requires_approval(tool_name) {
        return ToolApproval::Auto;
    }

    // Step 4: FullAuto mode → Auto.
    if ctx.mode == ApprovalMode::FullAuto {
        return ToolApproval::Auto;
    }

    // Step 5: User allow rules.
    for rule in ctx.allow_rules {
        if matches_rule(tool_name, arguments, rule, cwd, true) {
            return ToolApproval::Auto;
        }
    }

    // Step 6: Session cache.
    if tool_name == "shell"
        && let Some(command) = extract_shell_command(arguments)
        && let Some(prefixes) = krew_tools::builtin::extract_command_prefixes(&command)
    {
        let mut all_cached = !prefixes.is_empty();
        for p in &prefixes {
            let key = format!("shell:{p}");
            if !ctx.cache.is_approved(&key).await {
                all_cached = false;
                break;
            }
        }
        if all_cached {
            return ToolApproval::Auto;
        }
    } else if tool_name == "fetch_url"
        && let Some(host) = extract_fetch_host(arguments)
        && ctx.cache.is_approved(&format!("fetch_url:{host}")).await
    {
        return ToolApproval::Auto;
    } else if ctx.cache.is_approved(tool_name).await {
        return ToolApproval::Auto;
    }

    // Step 7: AutoEdit + write tools → Auto.
    if ctx.mode == ApprovalMode::AutoEdit && tool_name != "shell" && tool_name != "fetch_url" {
        return ToolApproval::Auto;
    }

    // Step 8: Default → NeedsApproval.
    let allow_session = if tool_name == "shell" {
        // Complex commands cannot be cached reliably.
        extract_shell_command(arguments)
            .and_then(|cmd| krew_tools::builtin::extract_command_prefixes(&cmd))
            .is_some()
    } else {
        true
    };

    ToolApproval::NeedsApproval {
        allow_session_approval: allow_session,
        reason: None,
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

    fn default_ctx<'a>(
        registry: &'a ToolRegistry,
        mode: ApprovalMode,
        cache: &'a ApprovalCache,
    ) -> ApprovalContext<'a> {
        ApprovalContext {
            tools: registry,
            mode,
            cache,
            allow_rules: &[],
            deny_rules: &[],
            ask_rules: &[],
            cwd: "/tmp",
        }
    }

    /// Helper to check approval result.
    async fn is_auto(
        tool_name: &str,
        arguments: &str,
        registry: &ToolRegistry,
        mode: ApprovalMode,
        cache: &ApprovalCache,
    ) -> bool {
        let ctx = default_ctx(registry, mode, cache);
        matches!(
            check_tool_approval(tool_name, arguments, &ctx).await,
            ToolApproval::Auto
        )
    }

    #[tokio::test]
    async fn suggest_mode_readonly_auto() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        assert!(
            is_auto(
                "read_file",
                r#"{"file_path":"src/main.rs"}"#,
                &registry,
                ApprovalMode::Suggest,
                &cache
            )
            .await
        );
        assert!(is_auto("glob", "{}", &registry, ApprovalMode::Suggest, &cache).await);
        assert!(is_auto("grep", "{}", &registry, ApprovalMode::Suggest, &cache).await);
    }

    #[tokio::test]
    async fn suggest_mode_write_needs_approval() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        assert!(
            !is_auto(
                "write_file",
                r#"{"file_path":"foo.txt"}"#,
                &registry,
                ApprovalMode::Suggest,
                &cache
            )
            .await
        );
        assert!(
            !is_auto(
                "edit_file",
                r#"{"file_path":"foo.txt"}"#,
                &registry,
                ApprovalMode::Suggest,
                &cache
            )
            .await
        );
        let shell_args = r#"{"command":"rm -rf /tmp/junk"}"#;
        assert!(
            !is_auto(
                "shell",
                shell_args,
                &registry,
                ApprovalMode::Suggest,
                &cache
            )
            .await
        );
    }

    #[tokio::test]
    async fn auto_edit_mode_write_auto() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        assert!(
            is_auto(
                "write_file",
                r#"{"file_path":"foo.txt"}"#,
                &registry,
                ApprovalMode::AutoEdit,
                &cache
            )
            .await
        );
        assert!(
            is_auto(
                "edit_file",
                r#"{"file_path":"foo.txt"}"#,
                &registry,
                ApprovalMode::AutoEdit,
                &cache
            )
            .await
        );
    }

    #[tokio::test]
    async fn auto_edit_mode_shell_needs_approval() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let shell_args = r#"{"command":"rm -rf /tmp/junk"}"#;
        assert!(
            !is_auto(
                "shell",
                shell_args,
                &registry,
                ApprovalMode::AutoEdit,
                &cache
            )
            .await
        );
    }

    #[tokio::test]
    async fn full_auto_mode_all_auto() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        assert!(
            is_auto(
                "read_file",
                r#"{"file_path":"src/main.rs"}"#,
                &registry,
                ApprovalMode::FullAuto,
                &cache
            )
            .await
        );
        assert!(
            is_auto(
                "write_file",
                r#"{"file_path":"foo.txt"}"#,
                &registry,
                ApprovalMode::FullAuto,
                &cache
            )
            .await
        );
        assert!(
            is_auto(
                "edit_file",
                r#"{"file_path":"foo.txt"}"#,
                &registry,
                ApprovalMode::FullAuto,
                &cache
            )
            .await
        );
        let shell_args = r#"{"command":"echo hello"}"#;
        assert!(
            is_auto(
                "shell",
                shell_args,
                &registry,
                ApprovalMode::FullAuto,
                &cache
            )
            .await
        );
    }

    #[tokio::test]
    async fn unknown_tool_no_approval() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        assert!(
            is_auto(
                "unknown_tool",
                "{}",
                &registry,
                ApprovalMode::Suggest,
                &cache
            )
            .await
        );
    }

    // ── Deny rules ──

    #[tokio::test]
    async fn deny_rule_blocks_shell() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let deny = vec![PermissionRule {
            tool: "shell".into(),
            pattern: Some("rm *".into()),
            reason: Some("Deletion not allowed".into()),
        }];
        let ctx = ApprovalContext {
            tools: &registry,
            mode: ApprovalMode::FullAuto,
            cache: &cache,
            allow_rules: &[],
            deny_rules: &deny,
            ask_rules: &[],
            cwd: "/tmp",
        };
        let result = check_tool_approval("shell", r#"{"command":"rm foo.txt"}"#, &ctx).await;
        match result {
            ToolApproval::Denied { reason } => assert_eq!(reason, "Deletion not allowed"),
            _ => panic!("expected Denied"),
        }
    }

    #[tokio::test]
    async fn deny_overrides_allow() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow = vec![PermissionRule {
            tool: "shell".into(),
            pattern: None,
            reason: None,
        }];
        let deny = vec![PermissionRule {
            tool: "shell".into(),
            pattern: Some("rm *".into()),
            reason: None,
        }];
        let ctx = ApprovalContext {
            tools: &registry,
            mode: ApprovalMode::Suggest,
            cache: &cache,
            allow_rules: &allow,
            deny_rules: &deny,
            ask_rules: &[],
            cwd: "/tmp",
        };
        let result = check_tool_approval("shell", r#"{"command":"rm foo.txt"}"#, &ctx).await;
        assert!(matches!(result, ToolApproval::Denied { .. }));
    }

    // ── Ask rules ──

    #[tokio::test]
    async fn ask_rule_forces_approval_in_full_auto() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let ask = vec![PermissionRule {
            tool: "shell".into(),
            pattern: Some("npm publish *".into()),
            reason: Some("Publishing needs confirmation".into()),
        }];
        let ctx = ApprovalContext {
            tools: &registry,
            mode: ApprovalMode::FullAuto,
            cache: &cache,
            allow_rules: &[],
            deny_rules: &[],
            ask_rules: &ask,
            cwd: "/tmp",
        };
        let result = check_tool_approval("shell", r#"{"command":"npm publish"}"#, &ctx).await;
        match result {
            ToolApproval::NeedsApproval { reason, .. } => {
                assert_eq!(reason.unwrap(), "Publishing needs confirmation")
            }
            _ => panic!("expected NeedsApproval"),
        }
    }

    // ── Allow rules ──

    #[tokio::test]
    async fn allow_rule_auto_approves() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow = vec![PermissionRule {
            tool: "shell".into(),
            pattern: Some("cargo *".into()),
            reason: None,
        }];
        let ctx = ApprovalContext {
            tools: &registry,
            mode: ApprovalMode::Suggest,
            cache: &cache,
            allow_rules: &allow,
            deny_rules: &[],
            ask_rules: &[],
            cwd: "/tmp",
        };
        let result =
            check_tool_approval("shell", r#"{"command":"cargo build --release"}"#, &ctx).await;
        assert!(matches!(result, ToolApproval::Auto));
    }

    // ── Bypass immunity ──

    async fn check_with_defaults(
        tool: &str,
        args: &str,
        registry: &ToolRegistry,
        mode: ApprovalMode,
        cache: &ApprovalCache,
    ) -> ToolApproval {
        let ctx = default_ctx(registry, mode, cache);
        check_tool_approval(tool, args, &ctx).await
    }

    #[tokio::test]
    async fn bypass_immunity_git_full_auto() {
        let r = test_registry();
        let c = ApprovalCache::new();
        assert!(matches!(
            check_with_defaults(
                "write_file",
                r#"{"file_path":".git/config"}"#,
                &r,
                ApprovalMode::FullAuto,
                &c
            )
            .await,
            ToolApproval::NeedsApproval { .. }
        ));
    }

    #[tokio::test]
    async fn bypass_immunity_krew_full_auto() {
        let r = test_registry();
        let c = ApprovalCache::new();
        assert!(matches!(
            check_with_defaults(
                "edit_file",
                r#"{"file_path":".krew/settings.toml"}"#,
                &r,
                ApprovalMode::FullAuto,
                &c
            )
            .await,
            ToolApproval::NeedsApproval { .. }
        ));
    }

    #[tokio::test]
    async fn bypass_immunity_env_read() {
        let r = test_registry();
        let c = ApprovalCache::new();
        assert!(matches!(
            check_with_defaults(
                "read_file",
                r#"{"file_path":".env"}"#,
                &r,
                ApprovalMode::FullAuto,
                &c
            )
            .await,
            ToolApproval::NeedsApproval { .. }
        ));
    }

    #[tokio::test]
    async fn bypass_immunity_not_bypassed_by_cache() {
        let r = test_registry();
        let c = ApprovalCache::new();
        c.approve_for_session("edit_file".to_string()).await;
        assert!(matches!(
            check_with_defaults(
                "edit_file",
                r#"{"file_path":".bashrc"}"#,
                &r,
                ApprovalMode::Suggest,
                &c
            )
            .await,
            ToolApproval::NeedsApproval { .. }
        ));
    }

    #[tokio::test]
    async fn normal_path_not_affected() {
        let r = test_registry();
        let c = ApprovalCache::new();
        assert!(
            is_auto(
                "write_file",
                r#"{"file_path":"src/main.rs"}"#,
                &r,
                ApprovalMode::FullAuto,
                &c
            )
            .await
        );
    }

    // ── Built-in shell deny ──

    #[tokio::test]
    async fn builtin_shell_deny_rm_git() {
        let r = test_registry();
        let c = ApprovalCache::new();
        assert!(matches!(
            check_with_defaults(
                "shell",
                r#"{"command":"rm -rf .git"}"#,
                &r,
                ApprovalMode::FullAuto,
                &c
            )
            .await,
            ToolApproval::Denied { .. }
        ));
    }

    #[tokio::test]
    async fn builtin_shell_deny_rm_env() {
        let r = test_registry();
        let c = ApprovalCache::new();
        assert!(matches!(
            check_with_defaults(
                "shell",
                r#"{"command":"rm .env"}"#,
                &r,
                ApprovalMode::FullAuto,
                &c
            )
            .await,
            ToolApproval::Denied { .. }
        ));
    }

    #[tokio::test]
    async fn builtin_shell_deny_redirect_env() {
        let r = test_registry();
        let c = ApprovalCache::new();
        assert!(matches!(
            check_with_defaults(
                "shell",
                r#"{"command":"echo x > .env"}"#,
                &r,
                ApprovalMode::FullAuto,
                &c
            )
            .await,
            ToolApproval::Denied { .. }
        ));
    }

    #[tokio::test]
    async fn builtin_shell_deny_redirect_git_config() {
        let r = test_registry();
        let c = ApprovalCache::new();
        assert!(matches!(
            check_with_defaults(
                "shell",
                r#"{"command":"cat foo > .git/config"}"#,
                &r,
                ApprovalMode::FullAuto,
                &c
            )
            .await,
            ToolApproval::Denied { .. }
        ));
    }

    #[tokio::test]
    async fn builtin_shell_allows_normal_rm() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let args = r#"{"command":"rm -rf target/"}"#;
        // Should NOT be blocked by built-in deny.
        assert!(is_auto("shell", args, &registry, ApprovalMode::FullAuto, &cache).await);
    }

    // ── Session cache ──

    #[tokio::test]
    async fn shell_session_cache_by_prefix() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let args = r#"{"command":"cargo build --release"}"#;
        assert!(!is_auto("shell", args, &registry, ApprovalMode::Suggest, &cache).await);

        cache_session_approval("shell", args, &cache).await;
        assert!(is_auto("shell", args, &registry, ApprovalMode::Suggest, &cache).await);

        let args2 = r#"{"command":"cargo build -p krew-core"}"#;
        assert!(is_auto("shell", args2, &registry, ApprovalMode::Suggest, &cache).await);

        let args3 = r#"{"command":"cargo test"}"#;
        assert!(!is_auto("shell", args3, &registry, ApprovalMode::Suggest, &cache).await);
    }

    #[tokio::test]
    async fn fetch_url_session_cache_by_host() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let args = r#"{"url":"https://docs.rs/htmd/latest"}"#;
        assert!(!is_auto("fetch_url", args, &registry, ApprovalMode::Suggest, &cache).await);

        cache_session_approval("fetch_url", args, &cache).await;
        let args2 = r#"{"url":"https://docs.rs/serde/latest"}"#;
        assert!(is_auto("fetch_url", args2, &registry, ApprovalMode::Suggest, &cache).await);

        let args3 = r#"{"url":"https://evil.com/page"}"#;
        assert!(!is_auto("fetch_url", args3, &registry, ApprovalMode::Suggest, &cache).await);
    }

    // ── is_dangerous_path ──

    #[test]
    fn dangerous_path_git() {
        assert!(is_dangerous_path(".git/config"));
        assert!(is_dangerous_path(".git/refs/heads/main"));
    }

    #[test]
    fn dangerous_path_krew() {
        assert!(is_dangerous_path(".krew/settings.toml"));
    }

    #[test]
    fn dangerous_path_files() {
        assert!(is_dangerous_path(".bashrc"));
        assert!(is_dangerous_path(".env"));
        assert!(is_dangerous_path(".gitconfig"));
    }

    #[test]
    fn dangerous_path_normal() {
        assert!(!is_dangerous_path("src/main.rs"));
        assert!(!is_dangerous_path("tests/foo.rs"));
    }

    #[test]
    fn dangerous_path_windows() {
        assert!(is_dangerous_path(".git\\config"));
    }

    #[test]
    fn dangerous_path_case_insensitive() {
        assert!(is_dangerous_path(".Git/config"));
        assert!(is_dangerous_path(".GIT/config"));
    }

    // ── Wildcard matching ──

    #[test]
    fn wildcard_basic() {
        assert!(match_shell_wildcard(
            "cargo build *",
            "cargo build --release"
        ));
        assert!(!match_shell_wildcard("cargo build *", "cargo test"));
    }

    #[test]
    fn wildcard_trailing_optional() {
        assert!(match_shell_wildcard("cargo *", "cargo"));
        assert!(match_shell_wildcard("cargo *", "cargo build"));
    }

    #[test]
    fn wildcard_escaped_star() {
        assert!(match_shell_wildcard("echo \\*", "echo *"));
        assert!(!match_shell_wildcard("echo \\*", "echo hello"));
    }

    // ── Glob matching ──

    #[test]
    fn glob_doublestar() {
        assert!(match_file_glob("src/**", "src/core/mod.rs"));
        assert!(match_file_glob("src/**", "src/main.rs"));
    }

    #[test]
    fn glob_single_star_no_recurse() {
        assert!(!match_file_glob("src/*", "src/core/mod.rs"));
        assert!(match_file_glob("src/*", "src/main.rs"));
    }

    #[test]
    fn glob_exact() {
        assert!(match_file_glob(".env", ".env"));
        assert!(!match_file_glob(".env", ".env.local"));
    }

    // ── Path normalization ──

    #[test]
    fn normalize_absolute() {
        assert_eq!(
            normalize_file_path("/home/user/project/.env", "/home/user/project"),
            ".env"
        );
    }

    #[test]
    fn normalize_dotdot() {
        assert_eq!(normalize_file_path("src/../.env", "/tmp"), ".env");
    }

    #[test]
    fn normalize_leading_dot_slash() {
        assert_eq!(normalize_file_path("./.git/config", "/tmp"), ".git/config");
    }

    #[test]
    fn normalize_windows() {
        assert_eq!(
            normalize_file_path(
                "G:\\AI\\Work\\project\\src\\main.rs",
                "G:\\AI\\Work\\project"
            ),
            "src/main.rs"
        );
    }

    // ── Domain suffix ──

    #[test]
    fn domain_suffix_match() {
        assert!(match_domain_suffix("github.com", "github.com"));
        assert!(match_domain_suffix("github.com", "docs.github.com"));
        assert!(!match_domain_suffix("github.com", "gitlab.com"));
    }

    // ── Compound shell command matching ──

    #[test]
    fn deny_matches_any_segment() {
        // deny pattern matches second segment.
        assert!(matches_shell_rule(
            "git status && rm foo.txt",
            "rm *",
            false
        ));
    }

    #[test]
    fn allow_requires_all_segments() {
        // allow pattern only matches first segment.
        assert!(!matches_shell_rule(
            "git status && rm foo.txt",
            "git status *",
            true
        ));
    }

    #[test]
    fn complex_command_allow_fails() {
        // Complex construct: allow should not match.
        assert!(!matches_shell_rule("echo $(rm -rf /)", "echo *", true));
    }

    #[test]
    fn complex_command_deny_whole_string() {
        // Complex construct: deny does whole-string matching.
        assert!(matches_shell_rule("echo $(rm -rf /)", "*rm -rf*", false));
    }
}
