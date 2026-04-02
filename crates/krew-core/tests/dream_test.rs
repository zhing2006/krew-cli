use krew_core::command::{DreamScope, SlashCommand};
use krew_core::dream;

// ── Task 4.1: SlashCommand::from_input parsing ─────────────────────

#[test]
fn dream_parse_complete_command() {
    let cmd = SlashCommand::from_input("/dream global @opus");
    assert!(
        matches!(cmd, Some(SlashCommand::Dream(DreamScope::Global, ref name)) if name == "opus")
    );
}

#[test]
fn dream_parse_agent_scope() {
    let cmd = SlashCommand::from_input("/dream agent @sonnet");
    assert!(
        matches!(cmd, Some(SlashCommand::Dream(DreamScope::Agent, ref name)) if name == "sonnet")
    );
}

#[test]
fn dream_parse_all_scope() {
    let cmd = SlashCommand::from_input("/dream all @opus");
    assert!(matches!(cmd, Some(SlashCommand::Dream(DreamScope::All, ref name)) if name == "opus"));
}

#[test]
fn dream_parse_no_args_returns_empty_agent() {
    let cmd = SlashCommand::from_input("/dream");
    assert!(matches!(cmd, Some(SlashCommand::Dream(_, ref name)) if name.is_empty()));
}

#[test]
fn dream_parse_missing_agent_returns_empty() {
    let cmd = SlashCommand::from_input("/dream global");
    assert!(
        matches!(cmd, Some(SlashCommand::Dream(DreamScope::Global, ref name)) if name.is_empty())
    );
}

#[test]
fn dream_parse_at_all_returns_all_string() {
    // @all is parsed as agent name "all"; rejection happens at execution time.
    let cmd = SlashCommand::from_input("/dream agent @all");
    assert!(matches!(cmd, Some(SlashCommand::Dream(DreamScope::Agent, ref name)) if name == "all"));
}

#[test]
fn dream_parse_invalid_scope_returns_empty_agent() {
    let cmd = SlashCommand::from_input("/dream invalid @opus");
    assert!(matches!(cmd, Some(SlashCommand::Dream(_, ref name)) if name.is_empty()));
}

#[test]
fn dream_name_is_slash_dream() {
    let cmd = SlashCommand::from_input("/dream global @opus").unwrap();
    assert_eq!(cmd.name(), "/dream");
}

#[test]
fn all_help_contains_dream() {
    let entries = SlashCommand::all_help();
    assert!(entries.iter().any(|(name, _)| *name == "/dream"));
}

// ── Task 4.2: DreamScope ────────────────────────────────────────────

#[test]
fn dream_scope_global_eq() {
    assert_eq!(DreamScope::Global, DreamScope::Global);
    assert_ne!(DreamScope::Global, DreamScope::Agent);
}

#[test]
fn dream_scope_debug() {
    // DreamScope derives Debug.
    let s = format!("{:?}", DreamScope::Agent);
    assert_eq!(s, "Agent");
}

// ── Task 4.3: build_dream_prompt ────────────────────────────────────

#[test]
fn dream_prompt_global_scope_contains_global_dir() {
    let prompt = dream::build_dream_prompt(DreamScope::Global, "opus");
    assert!(prompt.contains(".krew/memory/"));
    assert!(!prompt.contains(".krew/memory/agents/opus/"));
    assert!(prompt.contains("Phase 1"));
    assert!(prompt.contains("Phase 2"));
    assert!(prompt.contains("Phase 3"));
    assert!(prompt.contains("glob"));
}

#[test]
fn dream_prompt_agent_scope_contains_agent_dir() {
    let prompt = dream::build_dream_prompt(DreamScope::Agent, "opus");
    assert!(prompt.contains(".krew/memory/agents/opus/"));
    // Should not contain the global dir description (but "global" appears in scope label)
    assert!(!prompt.contains("Global memory"));
}

#[test]
fn dream_prompt_all_scope_contains_both_dirs() {
    let prompt = dream::build_dream_prompt(DreamScope::All, "opus");
    assert!(prompt.contains(".krew/memory/"));
    assert!(prompt.contains(".krew/memory/agents/opus/"));
    assert!(prompt.contains("independent indexes"));
}

#[test]
fn dream_prompt_has_three_phases() {
    let prompt = dream::build_dream_prompt(DreamScope::Global, "test");
    assert!(prompt.contains("Phase 1"));
    assert!(prompt.contains("Phase 2"));
    assert!(prompt.contains("Phase 3"));
}

#[test]
fn dream_prompt_uses_glob_not_ls() {
    let prompt = dream::build_dream_prompt(DreamScope::Global, "test");
    assert!(prompt.contains("glob"));
    assert!(!prompt.contains(" ls "));
}

#[test]
fn dream_allowed_tools_whitelist() {
    assert!(dream::DREAM_ALLOWED_TOOLS.contains(&"read_file"));
    assert!(dream::DREAM_ALLOWED_TOOLS.contains(&"write_file"));
    assert!(dream::DREAM_ALLOWED_TOOLS.contains(&"edit_file"));
    assert!(dream::DREAM_ALLOWED_TOOLS.contains(&"glob"));
    assert!(dream::DREAM_ALLOWED_TOOLS.contains(&"grep"));
    assert!(!dream::DREAM_ALLOWED_TOOLS.contains(&"shell"));
    assert!(!dream::DREAM_ALLOWED_TOOLS.contains(&"fetch_url"));
    assert!(!dream::DREAM_ALLOWED_TOOLS.contains(&"activate_skill"));
    assert!(!dream::DREAM_ALLOWED_TOOLS.contains(&"run_agent"));
}
