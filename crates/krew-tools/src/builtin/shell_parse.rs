//! Shell command parsing for approval-level granularity.
//!
//! Extracts "command prefixes" (e.g. `cargo build`, `git status`) from
//! shell command strings to enable per-command approval decisions.

use std::path::Path;

/// Extract command prefixes from a shell command string.
///
/// Splits the command by shell operators (`&&`, `||`, `;`, `|`) and extracts
/// the command identity (command name + optional subcommand) from each segment.
///
/// Returns `None` if the command contains complex constructs that prevent
/// reliable parsing (backticks, `$()`, redirections, variable expansion, etc.).
/// When `None` is returned, the caller should disable "approve for session"
/// since we cannot reliably identify the command.
///
/// # Examples
///
/// ```
/// use krew_tools::builtin::extract_command_prefixes;
///
/// assert_eq!(
///     extract_command_prefixes("cargo build --release"),
///     Some(vec!["cargo build".to_string()]),
/// );
/// assert_eq!(
///     extract_command_prefixes("ls -la && echo done"),
///     Some(vec!["ls".to_string(), "echo done".to_string()]),
/// );
/// assert_eq!(
///     extract_command_prefixes("git status"),
///     Some(vec!["git status".to_string()]),
/// );
/// // Complex command — cannot reliably parse.
/// assert_eq!(extract_command_prefixes("echo $(whoami)"), None);
/// ```
pub fn extract_command_prefixes(command: &str) -> Option<Vec<String>> {
    // Reject commands with complex shell constructs that we cannot
    // reliably decompose into simple command invocations.
    if has_complex_constructs(command) {
        return None;
    }

    let segments = split_shell_operators(command);
    let mut prefixes = Vec::new();

    for segment in &segments {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        if let Some(prefix) = extract_single_prefix(segment) {
            prefixes.push(prefix);
        } else {
            // Could not extract a meaningful prefix from this segment.
            return None;
        }
    }

    if prefixes.is_empty() {
        return None;
    }

    Some(prefixes)
}

/// Check whether a command prefix matches an allowlist entry.
///
/// An allowlist entry matches a command prefix when the entry's tokens
/// are a prefix of the command prefix's tokens:
///
/// - `"cargo"` matches `"cargo build"`, `"cargo test"`, etc.
/// - `"cargo build"` matches only `"cargo build"`.
/// - `"git status"` matches `"git status"` but not `"git push"`.
pub fn matches_allowlist_entry(command_prefix: &str, entry: &str) -> bool {
    let entry_tokens: Vec<&str> = entry.split_whitespace().collect();
    let prefix_tokens: Vec<&str> = command_prefix.split_whitespace().collect();

    if entry_tokens.is_empty() || prefix_tokens.len() < entry_tokens.len() {
        return false;
    }

    entry_tokens
        .iter()
        .zip(prefix_tokens.iter())
        .all(|(e, p)| *e == *p)
}

// ── Internal helpers ────────────────────────────────────────────────

/// Detect complex shell constructs that prevent reliable command extraction.
fn has_complex_constructs(command: &str) -> bool {
    let chars: Vec<char> = command.chars().collect();
    let len = chars.len();

    for (i, &ch) in chars.iter().enumerate() {
        match ch {
            // Backtick command substitution.
            '`' => return true,
            // $() command substitution or $VAR expansion.
            '$' => {
                if i + 1 < len {
                    let next = chars[i + 1];
                    if next == '(' || next == '{' || next.is_alphanumeric() || next == '_' {
                        return true;
                    }
                }
            }
            // Redirections.
            '>' | '<' => return true,
            // Subshell.
            '(' | ')' => return true,
            // Brace expansion / command grouping.
            '{' | '}' => return true,
            _ => {}
        }
    }

    false
}

/// Split a command string by shell operators `&&`, `||`, `;`, `|`.
///
/// Respects quoted strings (single and double quotes) so that operators
/// inside quotes are not treated as separators.
fn split_shell_operators(command: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = command.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    while i < len {
        let ch = chars[i];

        // Track quote state.
        if ch == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            current.push(ch);
            i += 1;
            continue;
        }
        if ch == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            current.push(ch);
            i += 1;
            continue;
        }

        // Inside quotes: no splitting.
        if in_single_quote || in_double_quote {
            current.push(ch);
            i += 1;
            continue;
        }

        // Check for shell operators.
        match ch {
            '&' if i + 1 < len && chars[i + 1] == '&' => {
                segments.push(std::mem::take(&mut current));
                i += 2;
            }
            '|' if i + 1 < len && chars[i + 1] == '|' => {
                segments.push(std::mem::take(&mut current));
                i += 2;
            }
            '|' => {
                segments.push(std::mem::take(&mut current));
                i += 1;
            }
            ';' => {
                segments.push(std::mem::take(&mut current));
                i += 1;
            }
            _ => {
                current.push(ch);
                i += 1;
            }
        }
    }

    if !current.is_empty() {
        segments.push(current);
    }

    segments
}

/// Extract a command prefix from a single command segment (no operators).
///
/// Returns the first one or two meaningful tokens as the "command identity":
/// - Skips environment variable assignments (`FOO=bar`)
/// - Skips `sudo` prefix
/// - Takes the command name (basename without extension)
/// - Takes the next non-flag token as subcommand (if any)
fn extract_single_prefix(segment: &str) -> Option<String> {
    let tokens: Vec<&str> = segment.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    let mut iter = tokens.iter().peekable();

    // Skip environment variable assignments at the start (FOO=bar).
    while let Some(&&token) = iter.peek() {
        if token.contains('=') && !token.starts_with('-') {
            iter.next();
        } else {
            break;
        }
    }

    // Get the command name.
    let raw_cmd = match iter.next() {
        Some(&token) => token,
        None => return None,
    };

    // Skip sudo — the real command follows.
    let raw_cmd = if raw_cmd == "sudo" {
        // Skip sudo flags. Some flags take an argument (e.g. -u root).
        loop {
            match iter.peek() {
                Some(&&t) if t.starts_with('-') => {
                    let flag = *iter.next().unwrap();
                    // Flags that consume the next token as their argument.
                    if matches!(flag, "-u" | "-g" | "-C" | "-D" | "-p" | "-r" | "-t") {
                        iter.next();
                    }
                }
                _ => break,
            }
        }
        match iter.next() {
            Some(&token) => token,
            None => return None,
        }
    } else {
        raw_cmd
    };

    let cmd = normalize_executable(raw_cmd);

    // Look for a subcommand: the next token that looks like a
    // subcommand word (e.g. "build", "status", "install").
    let subcommand = iter.find(|&&t| looks_like_subcommand(t)).copied();

    match subcommand {
        Some(sub) => Some(format!("{cmd} {sub}")),
        None => Some(cmd),
    }
}

/// Check if a token looks like a subcommand (e.g. "build", "status", "install")
/// rather than a file path or argument value.
fn looks_like_subcommand(token: &str) -> bool {
    !token.is_empty()
        && !token.starts_with('-')         // not a flag
        && !token.starts_with('\'')        // not a quoted string
        && !token.starts_with('"')
        && !token.contains('/')            // not a Unix path
        && !token.contains('\\')           // not a Windows path
        && !token.contains('.')            // not a filename with extension
        && !token.contains('=')            // not an assignment
        && token.len() <= 20               // reasonable subcommand length
        && token
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == ':')
}

/// Normalize an executable name: strip path and Windows extensions.
fn normalize_executable(raw: &str) -> String {
    let name = Path::new(raw)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(raw);

    #[cfg(windows)]
    {
        let lower = name.to_ascii_lowercase();
        for suffix in [".exe", ".cmd", ".bat", ".com"] {
            if let Some(stripped) = lower.strip_suffix(suffix) {
                return stripped.to_string();
            }
        }
        lower
    }

    #[cfg(not(windows))]
    {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── extract_command_prefixes ─────────────────────────────────────

    #[test]
    fn simple_command() {
        assert_eq!(
            extract_command_prefixes("ls -la"),
            Some(vec!["ls".to_string()]),
        );
    }

    #[test]
    fn command_with_subcommand() {
        assert_eq!(
            extract_command_prefixes("cargo build --release"),
            Some(vec!["cargo build".to_string()]),
        );
    }

    #[test]
    fn git_status() {
        assert_eq!(
            extract_command_prefixes("git status"),
            Some(vec!["git status".to_string()]),
        );
    }

    #[test]
    fn chained_commands() {
        assert_eq!(
            extract_command_prefixes("ls -la && echo done"),
            Some(vec!["ls".to_string(), "echo done".to_string()]),
        );
    }

    #[test]
    fn piped_commands() {
        // file.txt and pattern are not subcommands (contain dot / just args).
        assert_eq!(
            extract_command_prefixes("cat file.txt | grep pattern"),
            Some(vec!["cat".to_string(), "grep pattern".to_string()]),
        );
    }

    #[test]
    fn env_var_prefix() {
        assert_eq!(
            extract_command_prefixes("FOO=bar cargo test"),
            Some(vec!["cargo test".to_string()]),
        );
    }

    #[test]
    fn sudo_prefix() {
        // /tmp/x is a path, not a subcommand.
        assert_eq!(
            extract_command_prefixes("sudo rm -rf /tmp/x"),
            Some(vec!["rm".to_string()]),
        );
    }

    #[test]
    fn sudo_with_flags() {
        assert_eq!(
            extract_command_prefixes("sudo -u root cargo build"),
            Some(vec!["cargo build".to_string()]),
        );
    }

    #[test]
    fn semicolon_separator() {
        assert_eq!(
            extract_command_prefixes("echo a; echo b"),
            Some(vec!["echo a".to_string(), "echo b".to_string()]),
        );
    }

    #[test]
    fn or_operator() {
        assert_eq!(
            extract_command_prefixes("cargo test || echo failed"),
            Some(vec!["cargo test".to_string(), "echo failed".to_string()]),
        );
    }

    #[test]
    fn complex_backtick() {
        assert_eq!(extract_command_prefixes("echo `whoami`"), None);
    }

    #[test]
    fn complex_command_substitution() {
        assert_eq!(extract_command_prefixes("echo $(whoami)"), None);
    }

    #[test]
    fn complex_variable_expansion() {
        assert_eq!(extract_command_prefixes("echo $HOME"), None);
    }

    #[test]
    fn complex_redirection() {
        assert_eq!(extract_command_prefixes("echo hello > file.txt"), None);
    }

    #[test]
    fn complex_input_redirection() {
        assert_eq!(extract_command_prefixes("cat < file.txt"), None);
    }

    #[test]
    fn complex_subshell() {
        assert_eq!(extract_command_prefixes("(echo hello)"), None);
    }

    #[test]
    fn quoted_string_not_split() {
        // Pipe inside quotes should not split the command.
        // Quoted tokens are treated as arguments, not subcommands.
        assert_eq!(
            extract_command_prefixes("echo 'hello | world'"),
            Some(vec!["echo".to_string()]),
        );
    }

    #[test]
    fn empty_command() {
        assert_eq!(extract_command_prefixes(""), None);
    }

    #[test]
    fn only_flags() {
        assert_eq!(
            extract_command_prefixes("ls --color --all"),
            Some(vec!["ls".to_string()]),
        );
    }

    // ── matches_allowlist_entry ──────────────────────────────────────

    #[test]
    fn allowlist_exact_match() {
        assert!(matches_allowlist_entry("ls", "ls"));
    }

    #[test]
    fn allowlist_prefix_match() {
        assert!(matches_allowlist_entry("cargo build", "cargo"));
    }

    #[test]
    fn allowlist_subcommand_match() {
        assert!(matches_allowlist_entry("cargo build", "cargo build"));
    }

    #[test]
    fn allowlist_subcommand_mismatch() {
        assert!(!matches_allowlist_entry("cargo test", "cargo build"));
    }

    #[test]
    fn allowlist_longer_entry_no_match() {
        assert!(!matches_allowlist_entry("cargo", "cargo build"));
    }

    #[test]
    fn allowlist_different_command() {
        assert!(!matches_allowlist_entry("npm install", "cargo"));
    }
}
