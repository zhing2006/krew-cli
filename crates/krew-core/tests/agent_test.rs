use krew_core::agent::build_system_prompt;

#[test]
fn instructions_and_system_prompt() {
    let result = build_system_prompt(
        Some("Use Rust conventions"),
        Some("You are a helpful assistant"),
    );
    assert_eq!(
        result.unwrap(),
        "<project-instructions>\nUse Rust conventions\n</project-instructions>\n\nYou are a helpful assistant"
    );
}

#[test]
fn instructions_without_system_prompt() {
    let result = build_system_prompt(Some("Use Rust conventions"), None);
    assert_eq!(
        result.unwrap(),
        "<project-instructions>\nUse Rust conventions\n</project-instructions>"
    );
}

#[test]
fn instructions_with_empty_system_prompt() {
    let result = build_system_prompt(Some("Use Rust conventions"), Some(""));
    assert_eq!(
        result.unwrap(),
        "<project-instructions>\nUse Rust conventions\n</project-instructions>"
    );
}

#[test]
fn no_instructions_with_system_prompt() {
    let result = build_system_prompt(None, Some("You are a helpful assistant"));
    assert_eq!(result.unwrap(), "You are a helpful assistant");
}

#[test]
fn no_instructions_no_system_prompt() {
    let result = build_system_prompt(None, None);
    assert!(result.is_none());
}
