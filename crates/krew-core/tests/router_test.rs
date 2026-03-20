use std::collections::VecDeque;

use krew_core::router::{
    Addressee, apply_immediate_routing, apply_queued_routing, parse_agent_mentions, parse_input,
};

fn agents() -> Vec<String> {
    vec!["gpt".to_string(), "opus".to_string()]
}

#[test]
fn parse_all() {
    let (addr, msg) = parse_input("@all hello", &agents()).unwrap();
    assert_eq!(addr, Addressee::All);
    assert_eq!(msg, "@all hello");
}

#[test]
fn parse_single_at_start() {
    let (addr, msg) = parse_input("@gpt explain this", &agents()).unwrap();
    assert_eq!(addr, Addressee::Single("gpt".to_string()));
    assert_eq!(msg, "@gpt explain this");
}

#[test]
fn parse_single_in_middle() {
    let (addr, msg) = parse_input("hey @gpt what do you think", &agents()).unwrap();
    assert_eq!(addr, Addressee::Single("gpt".to_string()));
    assert_eq!(msg, "hey @gpt what do you think");
}

#[test]
fn parse_single_at_end() {
    let (addr, msg) = parse_input("explain this @gpt", &agents()).unwrap();
    assert_eq!(addr, Addressee::Single("gpt".to_string()));
    assert_eq!(msg, "explain this @gpt");
}

#[test]
fn parse_multiple() {
    let (addr, msg) = parse_input("@gpt @opus debate this", &agents()).unwrap();
    assert_eq!(
        addr,
        Addressee::Multiple(vec!["gpt".to_string(), "opus".to_string()])
    );
    assert_eq!(msg, "@gpt @opus debate this");
}

#[test]
fn parse_multiple_scattered() {
    let (addr, msg) = parse_input("hey @gpt what does @opus think", &agents()).unwrap();
    assert_eq!(
        addr,
        Addressee::Multiple(vec!["gpt".to_string(), "opus".to_string()])
    );
    assert_eq!(msg, "hey @gpt what does @opus think");
}

#[test]
fn parse_all_mixed_with_specific() {
    let (addr, msg) = parse_input("@gpt @all hello", &agents()).unwrap();
    assert_eq!(addr, Addressee::All);
    assert_eq!(msg, "@gpt @all hello");
}

#[test]
fn parse_duplicate_deduped() {
    let (addr, msg) = parse_input("@gpt hello @gpt again", &agents()).unwrap();
    assert_eq!(addr, Addressee::Single("gpt".to_string()));
    assert_eq!(msg, "@gpt hello @gpt again");
}

#[test]
fn parse_unknown_agent_is_plain_text() {
    let (addr, msg) = parse_input("@unknown hello", &agents()).unwrap();
    assert_eq!(addr, Addressee::LastRespondent);
    assert_eq!(msg, "@unknown hello");
}

#[test]
fn parse_bare_at_is_plain_text() {
    let (addr, msg) = parse_input("@ hello", &agents()).unwrap();
    assert_eq!(addr, Addressee::LastRespondent);
    assert_eq!(msg, "@ hello");
}

#[test]
fn parse_mixed_known_and_unknown() {
    let (addr, msg) = parse_input("@gpt @unknown hello", &agents()).unwrap();
    assert_eq!(addr, Addressee::Single("gpt".to_string()));
    assert_eq!(msg, "@gpt @unknown hello");
}

#[test]
fn parse_no_prefix() {
    let (addr, msg) = parse_input("just chatting", &agents()).unwrap();
    assert_eq!(addr, Addressee::LastRespondent);
    assert_eq!(msg, "just chatting");
}

#[test]
fn parse_empty_fails() {
    assert!(parse_input("", &agents()).is_err());
}

// ── parse_agent_mentions tests ──────────────────────────────────────

fn all_agents() -> Vec<String> {
    vec!["gpt".to_string(), "opus".to_string(), "gemini".to_string()]
}

#[test]
fn mentions_basic_match() {
    let result = parse_agent_mentions("I think @opus should review this", &all_agents(), "gpt");
    assert_eq!(result, vec!["opus"]);
}

#[test]
fn mentions_self_excluded() {
    let result = parse_agent_mentions("Let me @gpt try again", &all_agents(), "gpt");
    assert!(result.is_empty());
}

#[test]
fn mentions_unknown_ignored() {
    let result = parse_agent_mentions("Hey @unknown check this", &all_agents(), "gpt");
    assert!(result.is_empty());
}

#[test]
fn mentions_all_ignored() {
    let result = parse_agent_mentions("@all please review", &all_agents(), "gpt");
    assert!(result.is_empty());
}

#[test]
fn mentions_multiple_returns_in_order() {
    let result = parse_agent_mentions("@gemini and @opus should discuss", &all_agents(), "gpt");
    assert_eq!(result, vec!["gemini", "opus"]);
}

#[test]
fn mentions_no_match_returns_empty() {
    let result = parse_agent_mentions("No mentions here", &all_agents(), "gpt");
    assert!(result.is_empty());
}

#[test]
fn mentions_trailing_comma() {
    let result = parse_agent_mentions("Hey @opus, what do you think?", &all_agents(), "gpt");
    assert_eq!(result, vec!["opus"]);
}

#[test]
fn mentions_trailing_colon_cjk() {
    let result = parse_agent_mentions("@opus：你觉得呢？", &all_agents(), "gpt");
    assert_eq!(result, vec!["opus"]);
}

#[test]
fn mentions_trailing_period() {
    let result = parse_agent_mentions("Ask @opus.", &all_agents(), "gpt");
    assert_eq!(result, vec!["opus"]);
}

#[test]
fn mentions_deduplicates() {
    let result = parse_agent_mentions("@opus hello @opus again", &all_agents(), "gpt");
    assert_eq!(result, vec!["opus"]);
}

#[test]
fn mentions_non_ascii_agent_name() {
    let agents = vec!["助手".to_string(), "opus".to_string()];
    let result = parse_agent_mentions("请 @助手 来看看", &agents, "opus");
    assert_eq!(result, vec!["助手"]);
}

#[test]
fn mentions_non_ascii_with_cjk_punctuation() {
    let agents = vec!["助手".to_string(), "opus".to_string()];
    let result = parse_agent_mentions("@助手，你觉得呢？", &agents, "opus");
    assert_eq!(result, vec!["助手"]);
}

#[test]
fn mentions_parse_still_works_when_feature_would_be_disabled() {
    // parse_agent_mentions itself is a pure function — the max_rounds=0 check
    // lives in the caller (App::handle_agent_event). This test verifies the
    // function returns results regardless, so the caller can decide.
    let result = parse_agent_mentions("@opus review this", &all_agents(), "gpt");
    assert_eq!(result, vec!["opus"]);
    // The caller is responsible for skipping routing when max_rounds == 0.
}

#[test]
fn mentions_prefix_no_false_match() {
    // "@opusX" should NOT match "opus" because 'X' is alphanumeric.
    let agents = vec!["opus".to_string(), "opusX".to_string()];
    let result = parse_agent_mentions("Hey @opusX check this", &agents, "gpt");
    assert_eq!(result, vec!["opusX"]);
}

#[test]
fn mentions_longest_prefix_wins() {
    // "@foo-bar" should match "foo-bar", not "foo" (longest wins).
    let agents = vec!["foo".to_string(), "foo-bar".to_string()];
    let result = parse_agent_mentions("Hey @foo-bar, check this", &agents, "gpt");
    assert_eq!(result, vec!["foo-bar"]);
}

#[test]
fn mentions_short_name_still_works_alone() {
    // "@foo," should match "foo" when "foo-bar" also exists.
    let agents = vec!["foo".to_string(), "foo-bar".to_string()];
    let result = parse_agent_mentions("Hey @foo, check this", &agents, "gpt");
    assert_eq!(result, vec!["foo"]);
}

// ── apply_immediate_routing tests ───────────────────────────────────

#[test]
fn immediate_target_not_in_queue() {
    let mut q: VecDeque<String> = VecDeque::from(vec!["opus".into(), "gemini".into()]);
    apply_immediate_routing(&mut q, "gpt");
    assert_eq!(
        q,
        VecDeque::from(vec!["gpt".to_string(), "opus".into(), "gemini".into()])
    );
}

#[test]
fn immediate_target_in_queue_not_head() {
    let mut q: VecDeque<String> = VecDeque::from(vec!["opus".into(), "gemini".into()]);
    apply_immediate_routing(&mut q, "gemini");
    assert_eq!(q, VecDeque::from(vec!["gemini".to_string(), "opus".into()]));
}

#[test]
fn immediate_target_already_at_head() {
    let mut q: VecDeque<String> = VecDeque::from(vec!["opus".into(), "gemini".into()]);
    apply_immediate_routing(&mut q, "opus");
    assert_eq!(q, VecDeque::from(vec!["opus".to_string(), "gemini".into()]));
}

#[test]
fn immediate_empty_queue() {
    let mut q: VecDeque<String> = VecDeque::new();
    apply_immediate_routing(&mut q, "opus");
    assert_eq!(q, VecDeque::from(vec!["opus".to_string()]));
}

// ── apply_queued_routing tests ──────────────────────────────────────

#[test]
fn queued_target_not_in_queue() {
    let mut q: VecDeque<String> = VecDeque::from(vec!["opus".into()]);
    apply_queued_routing(&mut q, "gemini");
    assert_eq!(q, VecDeque::from(vec!["opus".to_string(), "gemini".into()]));
}

#[test]
fn queued_target_already_in_queue() {
    let mut q: VecDeque<String> = VecDeque::from(vec!["opus".into(), "gemini".into()]);
    apply_queued_routing(&mut q, "opus");
    assert_eq!(q, VecDeque::from(vec!["opus".to_string(), "gemini".into()]));
}

#[test]
fn queued_empty_queue() {
    let mut q: VecDeque<String> = VecDeque::new();
    apply_queued_routing(&mut q, "opus");
    assert_eq!(q, VecDeque::from(vec!["opus".to_string()]));
}
