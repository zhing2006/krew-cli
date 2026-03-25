//! Tests for `krew config doctor` behavior.
//!
//! These tests verify that doctor reports TOML parse errors explicitly
//! rather than silently falling back to defaults, and handles normal
//! diagnostic scenarios correctly.

use std::process::Command;

fn krew_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_krew"))
}

#[test]
fn doctor_reports_missing_configs() {
    let dir = tempfile::tempdir().unwrap();

    let output = krew_bin()
        .args(["config", "doctor"])
        .current_dir(dir.path())
        .env("HOME", dir.path().join("nonexistent_home"))
        .env("USERPROFILE", dir.path().join("nonexistent_home"))
        .output()
        .expect("failed to run krew");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should report no config files found.
    assert!(
        stdout.contains("not found") || stdout.contains("No configuration files found"),
        "expected 'not found' message, got:\n{stdout}"
    );
}

#[test]
fn doctor_reports_corrupted_user_config() {
    let dir = tempfile::tempdir().unwrap();

    // Create a corrupted user config.
    let user_krew = dir.path().join(".krew");
    std::fs::create_dir_all(&user_krew).unwrap();
    std::fs::write(user_krew.join("settings.toml"), "this is [not valid toml\n").unwrap();

    let output = krew_bin()
        .args(["config", "doctor"])
        .current_dir(dir.path())
        .env("HOME", dir.path())
        .env("USERPROFILE", dir.path())
        .output()
        .expect("failed to run krew");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("parse error"),
        "expected parse error report for corrupted user config, got:\n{stdout}"
    );
}

#[test]
fn doctor_reports_corrupted_project_config() {
    let dir = tempfile::tempdir().unwrap();

    // Create a corrupted project config.
    let project_krew = dir.path().join(".krew");
    std::fs::create_dir_all(&project_krew).unwrap();
    std::fs::write(
        project_krew.join("settings.toml"),
        "[[agents]\nbroken toml\n",
    )
    .unwrap();

    let output = krew_bin()
        .args(["config", "doctor"])
        .current_dir(dir.path())
        .env("HOME", dir.path().join("nonexistent_home"))
        .env("USERPROFILE", dir.path().join("nonexistent_home"))
        .output()
        .expect("failed to run krew");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("parse error"),
        "expected parse error report for corrupted project config, got:\n{stdout}"
    );
}

#[test]
fn doctor_healthy_config() {
    let dir = tempfile::tempdir().unwrap();

    // Create user config.
    let user_krew = dir.path().join(".krew");
    std::fs::create_dir_all(&user_krew).unwrap();
    std::fs::write(
        user_krew.join("settings.toml"),
        "[providers.anthropic]\ntype = \"anthropic\"\napi_key = \"sk-test\"\n",
    )
    .unwrap();

    // Create project config.
    let project_krew = dir.path().join(".krew");
    std::fs::write(
        project_krew.join("settings.toml"),
        "[settings]\nreply_order = [\"claude\"]\n\n\
         [providers.anthropic]\ntype = \"anthropic\"\napi_key = \"sk-test\"\n\n\
         [[agents]]\nname = \"claude\"\ndisplay_name = \"Claude\"\nprovider = \"anthropic\"\n\
         model = \"claude-sonnet-4-6\"\ncolor = \"blue\"\nenable_thinking = true\nenable_web_search = false\n",
    )
    .unwrap();

    let output = krew_bin()
        .args(["config", "doctor"])
        .current_dir(dir.path())
        .env("HOME", dir.path())
        .env("USERPROFILE", dir.path())
        .output()
        .expect("failed to run krew");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Both configs should show as OK.
    assert!(
        stdout.contains("✅"),
        "expected at least one check pass, got:\n{stdout}"
    );
}

#[test]
fn doctor_missing_user_reports_not_found_not_cannot_verify() {
    let dir = tempfile::tempdir().unwrap();

    // Project config with an agent referencing a provider that only exists in
    // user config — but user config is simply missing (not corrupted).
    // Doctor should report "not found", NOT "cannot verify, user config unavailable".
    let project_krew = dir.path().join(".krew");
    std::fs::create_dir_all(&project_krew).unwrap();
    std::fs::write(
        project_krew.join("settings.toml"),
        "[settings]\nreply_order = [\"claude\"]\n\n\
         [[agents]]\nname = \"claude\"\ndisplay_name = \"Claude\"\n\
         provider = \"anthropic\"\nmodel = \"claude-sonnet-4-6\"\n\
         color = \"blue\"\nenable_thinking = true\nenable_web_search = false\n",
    )
    .unwrap();

    let output = krew_bin()
        .args(["config", "doctor"])
        .current_dir(dir.path())
        // Point HOME to a directory without .krew/ so user config is missing.
        .env("HOME", dir.path().join("no_user_home"))
        .env("USERPROFILE", dir.path().join("no_user_home"))
        .output()
        .expect("failed to run krew");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("provider: anthropic (not found)"),
        "expected 'not found' for missing provider when user config is absent, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("cannot verify"),
        "should NOT say 'cannot verify' when user config is simply missing, got:\n{stdout}"
    );
}
