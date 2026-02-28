use krew_config::{PROJECT_INSTRUCTIONS_MAX_SIZE, load_project_instructions};
use std::fs;

#[test]
fn no_file_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let result = load_project_instructions(dir.path()).unwrap();
    assert!(result.is_none());
}

#[test]
fn single_file_loaded() {
    let dir = tempfile::tempdir().unwrap();
    let content = "Use Rust conventions\nPrefer snake_case";
    fs::write(dir.path().join("AGENTS.md"), content).unwrap();

    let result = load_project_instructions(dir.path()).unwrap();
    assert_eq!(result.unwrap(), content);
}

#[test]
fn hierarchical_merge_ancestor_first() {
    let parent = tempfile::tempdir().unwrap();
    let child = parent.path().join("subdir");
    fs::create_dir_all(&child).unwrap();

    fs::write(parent.path().join("AGENTS.md"), "parent rules").unwrap();
    fs::write(child.join("AGENTS.md"), "child rules").unwrap();

    let result = load_project_instructions(&child).unwrap().unwrap();
    // Parent content should come before child content.
    let parent_pos = result.find("parent rules").unwrap();
    let child_pos = result.find("child rules").unwrap();
    assert!(parent_pos < child_pos);
    // Separated by blank line.
    assert!(result.contains("parent rules\n\nchild rules"));
}

#[test]
fn large_file_truncated() {
    let dir = tempfile::tempdir().unwrap();
    // Create a file larger than 100KB.
    let content = "x".repeat(PROJECT_INSTRUCTIONS_MAX_SIZE + 1000);
    fs::write(dir.path().join("AGENTS.md"), &content).unwrap();

    let result = load_project_instructions(dir.path()).unwrap().unwrap();
    assert!(result.contains("[WARNING: File truncated at 100KB limit]"));
    // The non-warning portion should be at most PROJECT_INSTRUCTIONS_MAX_SIZE bytes.
    let warning_suffix = "\n\n[WARNING: File truncated at 100KB limit]";
    let truncated_part = &result[..result.len() - warning_suffix.len()];
    assert!(truncated_part.len() <= PROJECT_INSTRUCTIONS_MAX_SIZE);
}

#[test]
fn non_utf8_file_skipped() {
    let dir = tempfile::tempdir().unwrap();
    // Write invalid UTF-8 bytes.
    fs::write(dir.path().join("AGENTS.md"), [0xFF, 0xFE, 0x80, 0x81]).unwrap();

    let result = load_project_instructions(dir.path()).unwrap();
    assert!(result.is_none());
}
