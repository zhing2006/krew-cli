use krew_core::agent::build_system_prompt;

#[test]
fn instructions_and_system_prompt() {
    let result = build_system_prompt(
        Some("Use Rust conventions"),
        None,
        Some("You are a helpful assistant"),
    );
    assert_eq!(
        result.unwrap(),
        "<project-instructions>\nUse Rust conventions\n</project-instructions>\n\nYou are a helpful assistant"
    );
}

#[test]
fn instructions_without_system_prompt() {
    let result = build_system_prompt(Some("Use Rust conventions"), None, None);
    assert_eq!(
        result.unwrap(),
        "<project-instructions>\nUse Rust conventions\n</project-instructions>"
    );
}

#[test]
fn instructions_with_empty_system_prompt() {
    let result = build_system_prompt(Some("Use Rust conventions"), None, Some(""));
    assert_eq!(
        result.unwrap(),
        "<project-instructions>\nUse Rust conventions\n</project-instructions>"
    );
}

#[test]
fn no_instructions_with_system_prompt() {
    let result = build_system_prompt(None, None, Some("You are a helpful assistant"));
    assert_eq!(result.unwrap(), "You are a helpful assistant");
}

#[test]
fn no_instructions_no_system_prompt() {
    let result = build_system_prompt(None, None, None);
    assert!(result.is_none());
}

#[test]
fn instructions_skills_and_system_prompt() {
    let catalog =
        "<available-skills>\n  <skill name=\"review\">Review code.</skill>\n</available-skills>";
    let result = build_system_prompt(Some("Use Rust"), Some(catalog), Some("You are helpful"));
    let output = result.unwrap();
    assert!(output.starts_with("<project-instructions>"));
    assert!(output.contains("<available-skills>"));
    assert!(output.ends_with("You are helpful"));
    // Verify order: project-instructions before skills before agent prompt.
    let pi_pos = output.find("<project-instructions>").unwrap();
    let sk_pos = output.find("<available-skills>").unwrap();
    let ap_pos = output.find("You are helpful").unwrap();
    assert!(pi_pos < sk_pos);
    assert!(sk_pos < ap_pos);
}

#[test]
fn skills_without_instructions() {
    let catalog =
        "<available-skills>\n  <skill name=\"test\">Test skill.</skill>\n</available-skills>";
    let result = build_system_prompt(None, Some(catalog), Some("Agent prompt"));
    let output = result.unwrap();
    assert!(output.starts_with("<available-skills>"));
    assert!(output.contains("Agent prompt"));
}

#[test]
fn empty_skill_catalog_ignored() {
    let result = build_system_prompt(None, Some(""), Some("Agent prompt"));
    assert_eq!(result.unwrap(), "Agent prompt");
}
