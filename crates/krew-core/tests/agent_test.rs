use krew_core::agent::{
    PeerAgent, build_identity_prompt, build_language_instruction, build_system_prompt,
};

#[test]
fn instructions_and_system_prompt() {
    let result = build_system_prompt(
        Some("Use Rust conventions"),
        None,
        None,
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
    let result = build_system_prompt(Some("Use Rust conventions"), None, None, None, None);
    assert_eq!(
        result.unwrap(),
        "<project-instructions>\nUse Rust conventions\n</project-instructions>"
    );
}

#[test]
fn instructions_with_empty_system_prompt() {
    let result = build_system_prompt(Some("Use Rust conventions"), None, None, None, Some(""));
    assert_eq!(
        result.unwrap(),
        "<project-instructions>\nUse Rust conventions\n</project-instructions>"
    );
}

#[test]
fn no_instructions_with_system_prompt() {
    let result = build_system_prompt(None, None, None, None, Some("You are a helpful assistant"));
    assert_eq!(result.unwrap(), "You are a helpful assistant");
}

#[test]
fn no_instructions_no_system_prompt() {
    let result = build_system_prompt(None, None, None, None, None);
    assert!(result.is_none());
}

#[test]
fn instructions_skills_and_system_prompt() {
    let catalog =
        "<available-skills>\n  <skill name=\"review\">Review code.</skill>\n</available-skills>";
    let result = build_system_prompt(
        Some("Use Rust"),
        Some(catalog),
        None,
        None,
        Some("You are helpful"),
    );
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
    let result = build_system_prompt(None, Some(catalog), None, None, Some("Agent prompt"));
    let output = result.unwrap();
    assert!(output.starts_with("<available-skills>"));
    assert!(output.contains("Agent prompt"));
}

#[test]
fn empty_skill_catalog_ignored() {
    let result = build_system_prompt(None, Some(""), None, None, Some("Agent prompt"));
    assert_eq!(result.unwrap(), "Agent prompt");
}

#[test]
fn memory_prompt_ordering() {
    let sub_agent_catalog = "<sub-agents>sub-agent list</sub-agents>";
    let memory = "## Global Memory\n\n- [User](user.md) — engineer";
    let result = build_system_prompt(
        Some("Project rules"),
        None,
        Some(sub_agent_catalog),
        Some(memory),
        Some("Agent prompt"),
    );
    let output = result.unwrap();
    // Verify order: sub-agent catalog → memory → agent prompt.
    let sa_pos = output.find("<sub-agents>").unwrap();
    let mem_pos = output.find("## Global Memory").unwrap();
    let ap_pos = output.find("Agent prompt").unwrap();
    assert!(sa_pos < mem_pos, "sub-agent catalog before memory");
    assert!(mem_pos < ap_pos, "memory before agent prompt");
}

#[test]
fn memory_prompt_only() {
    let memory = "## Global Memory\n\n- [User](user.md) — dev";
    let result = build_system_prompt(None, None, None, Some(memory), None);
    let output = result.unwrap();
    assert!(output.contains("## Global Memory"));
}

#[test]
fn empty_memory_prompt_ignored() {
    let result = build_system_prompt(None, None, None, Some(""), Some("Agent prompt"));
    assert_eq!(result.unwrap(), "Agent prompt");
}

#[test]
fn language_instruction_with_language_set() {
    let result = build_language_instruction(Some("中文"));
    assert_eq!(
        result,
        "\nAlways respond in 中文. Use 中文 for all explanations, comments, and communications with the user. Technical terms and code identifiers should remain in their original form."
    );
}

#[test]
fn language_instruction_without_language() {
    let result = build_language_instruction(None);
    assert!(result.is_empty());
}

// ── build_identity_prompt tests ─────────────────────────────────────

#[test]
fn identity_basic_no_language_no_peers() {
    let result = build_identity_prompt("GPT-5", "gpt-5", "gpt", "2026-03-24", None, None, None);
    assert!(result.contains("You are GPT-5, powered by the gpt-5 model."));
    assert!(result.contains("Current date/time: 2026-03-24"));
    // No language instruction.
    assert!(!result.contains("Always respond in"));
    // No peer or whisper hints.
    assert!(!result.contains("@name"));
    assert!(!result.contains("whisper"));
}

#[test]
fn identity_with_language() {
    let result = build_identity_prompt(
        "GPT-5",
        "gpt-5",
        "gpt",
        "2026-03-24",
        Some("中文"),
        None,
        None,
    );
    assert!(result.contains("Always respond in 中文."));
    assert!(
        result
            .contains("Technical terms and code identifiers should remain in their original form.")
    );
}

#[test]
fn identity_language_before_peer_hints() {
    let peers = [PeerAgent {
        name: "opus".to_string(),
        display_name: "Claude Opus".to_string(),
    }];
    let result = build_identity_prompt(
        "GPT-5",
        "gpt-5",
        "gpt",
        "2026-03-24",
        Some("中文"),
        Some(&peers),
        None,
    );
    let lang_pos = result.find("Always respond in 中文").unwrap();
    let peer_pos = result.find("Other agents:").unwrap();
    assert!(
        lang_pos < peer_pos,
        "language instruction should appear before peer agent hints"
    );
}

#[test]
fn identity_language_before_whisper() {
    let targets = vec!["gpt".to_string(), "opus".to_string()];
    let peers = [PeerAgent {
        name: "opus".to_string(),
        display_name: "Claude Opus".to_string(),
    }];
    let result = build_identity_prompt(
        "GPT-5",
        "gpt-5",
        "gpt",
        "2026-03-24",
        Some("中文"),
        Some(&peers),
        Some(&targets),
    );
    let lang_pos = result.find("Always respond in 中文").unwrap();
    let whisper_pos = result.find("private whisper conversation").unwrap();
    assert!(
        lang_pos < whisper_pos,
        "language instruction should appear before whisper context"
    );
}

#[test]
fn whisper_solo_includes_scope_and_confidentiality() {
    let targets = vec!["gpt".to_string()];
    let result = build_identity_prompt(
        "GPT-5",
        "gpt-5",
        "gpt",
        "2026-03-24",
        None,
        None,
        Some(&targets),
    );
    // Scope: AI knows this round is whisper-scoped.
    assert!(
        result.contains("Everything in this conversation round"),
        "should contain scope instruction"
    );
    // Confidentiality: AI must not leak in normal messages.
    assert!(
        result.contains("NEVER reveal"),
        "should contain confidentiality instruction"
    );
    // Solo whisper: can reference previous whispers with the same user.
    assert!(
        result.contains("another whisper with you"),
        "solo whisper should allow referencing previous whispers with the same user"
    );
}

#[test]
fn whisper_group_includes_scope_and_confidentiality() {
    let targets = vec!["gpt".to_string(), "opus".to_string()];
    let peers = [PeerAgent {
        name: "opus".to_string(),
        display_name: "Claude Opus".to_string(),
    }];
    let result = build_identity_prompt(
        "GPT-5",
        "gpt-5",
        "gpt",
        "2026-03-24",
        None,
        Some(&peers),
        Some(&targets),
    );
    // Scope instruction present.
    assert!(
        result.contains("Everything in this conversation round"),
        "should contain scope instruction"
    );
    // Confidentiality with group reconvene clause.
    assert!(
        result.contains("NEVER reveal"),
        "should contain confidentiality instruction"
    );
    assert!(
        result.contains("same whisper group"),
        "group whisper should mention same group reconvening"
    );
    assert!(
        result.contains("@opus"),
        "confidentiality should list group members"
    );
}

#[test]
fn identity_contains_krew_description_and_config_help() {
    let result = build_identity_prompt("GPT-5", "gpt-5", "gpt", "2026-03-24", None, None, None);
    assert!(
        result.contains("multi-AI-agent collaborative CLI tool"),
        "identity should contain krew description"
    );
    assert!(
        result.contains("krew config help"),
        "identity should contain config help hint"
    );
}

#[test]
fn identity_no_language_with_peers_and_whisper() {
    let targets = vec!["gpt".to_string()];
    let peers = [PeerAgent {
        name: "opus".to_string(),
        display_name: "Claude Opus".to_string(),
    }];
    let result = build_identity_prompt(
        "GPT-5",
        "gpt-5",
        "gpt",
        "2026-03-24",
        None,
        Some(&peers),
        Some(&targets),
    );
    // No language instruction injected.
    assert!(!result.contains("Always respond in"));
    // But peer and whisper hints are present.
    assert!(result.contains("Other agents:"));
    assert!(result.contains("private whisper conversation"));
}
