use krew_tools::builtin::{extract_command_prefixes, matches_allowlist_entry};

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
