use std::collections::VecDeque;

use krew_core::router::{
    Addressee, apply_immediate_routing, apply_immediate_routing_at, apply_queued_routing,
    parse_agent_mentions, parse_input,
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

#[test]
fn mentions_cjk_punctuation_before_at() {
    // "太好了！@gemini" — full-width '！' is not whitespace, '@' is mid-token.
    let result = parse_agent_mentions("太好了！@gemini 状态满分", &all_agents(), "opus");
    assert_eq!(result, vec!["gemini"]);
}

#[test]
fn mentions_cjk_punctuation_both_sides() {
    // "问问@opus，他怎么看" — no whitespace around @opus at all.
    let result = parse_agent_mentions("问问@opus，他怎么看", &all_agents(), "gemini");
    assert_eq!(result, vec!["opus"]);
}

#[test]
fn mentions_cjk_multiple_mentions() {
    // Multiple @mentions embedded in CJK text.
    let result = parse_agent_mentions("请@opus看看，然后让@gemini也来帮忙", &all_agents(), "gpt");
    assert_eq!(result, vec!["opus", "gemini"]);
}

#[test]
fn mentions_cjk_parentheses_around_at() {
    // Full-width parentheses around @mention: （@opus）
    let result = parse_agent_mentions("可以问问（@opus）的意见", &all_agents(), "gemini");
    assert_eq!(result, vec!["opus"]);
}

#[test]
fn mentions_cjk_mixed_with_ascii() {
    // Mixed CJK and ASCII @mentions in one text.
    let result = parse_agent_mentions("hi @opus！然后@gemini你觉得呢", &all_agents(), "gpt");
    assert_eq!(result, vec!["opus", "gemini"]);
}

#[test]
fn mentions_cjk_exclamation_before_at() {
    // Full-width exclamation mark directly before @: ！@opus
    let result = parse_agent_mentions("太棒了！@opus 你怎么看？", &all_agents(), "gemini");
    assert_eq!(result, vec!["opus"]);
}

#[test]
fn mentions_cjk_comma_after_at() {
    // Full-width comma after agent name: @opus，
    let result = parse_agent_mentions("@opus，你来回答这个问题", &all_agents(), "gpt");
    assert_eq!(result, vec!["opus"]);
}

#[test]
fn mentions_cjk_question_mark_after_at() {
    // Full-width question mark after agent name: @opus？
    let result = parse_agent_mentions("@opus？你在吗", &all_agents(), "gemini");
    assert_eq!(result, vec!["opus"]);
}

#[test]
fn mentions_cjk_no_space_start_of_text() {
    // @mention at very start of text followed by CJK without space.
    let result = parse_agent_mentions("@gemini你好啊", &all_agents(), "opus");
    assert_eq!(result, vec!["gemini"]);
}

#[test]
fn mentions_cjk_agent_name_in_sentence() {
    // Non-ASCII agent name embedded in CJK sentence without spaces.
    let agents = vec!["助手".to_string(), "opus".to_string()];
    let result = parse_agent_mentions("能不能让@助手来处理这个问题？", &agents, "opus");
    assert_eq!(result, vec!["助手"]);
}

#[test]
fn mentions_email_like_not_matched() {
    // "user@opus.com" — '@' preceded by alphanumeric should be skipped.
    let result = parse_agent_mentions("send to user@opus.com", &all_agents(), "gpt");
    assert!(result.is_empty());
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

// ── apply_immediate_routing_at tests ────────────────────────────────

#[test]
fn immediate_at_multi_targets_in_order() {
    // C mentions @a @b — both should be inserted at cursor, in order.
    let mut q: VecDeque<String> = VecDeque::new();
    let mut cursor: usize = 0;
    apply_immediate_routing_at(&mut q, "a", &mut cursor);
    apply_immediate_routing_at(&mut q, "b", &mut cursor);
    assert_eq!(q, VecDeque::from(vec!["a".to_string(), "b".into()]));
    assert_eq!(cursor, 2);
}

#[test]
fn immediate_at_no_starvation() {
    // C mentions @a @b, then A mentions @c — c should go after b, not before.
    let mut q: VecDeque<String> = VecDeque::new();
    let mut cursor: usize = 0;
    // C's turn: @a @b
    apply_immediate_routing_at(&mut q, "a", &mut cursor);
    apply_immediate_routing_at(&mut q, "b", &mut cursor);
    assert_eq!(q, VecDeque::from(vec!["a".to_string(), "b".into()]));
    // A pops from front
    q.pop_front();
    cursor = cursor.saturating_sub(1);
    // A's turn: @c
    apply_immediate_routing_at(&mut q, "c", &mut cursor);
    assert_eq!(q, VecDeque::from(vec!["b".to_string(), "c".into()]));
}

#[test]
fn immediate_at_target_already_in_queue() {
    // Target already in queue — move to cursor position.
    let mut q: VecDeque<String> = VecDeque::from(vec!["a".into(), "b".into(), "c".into()]);
    let mut cursor: usize = 1;
    apply_immediate_routing_at(&mut q, "c", &mut cursor);
    assert_eq!(
        q,
        VecDeque::from(vec!["a".to_string(), "c".into(), "b".into()])
    );
    assert_eq!(cursor, 2);
}

#[test]
fn immediate_at_target_already_at_cursor() {
    let mut q: VecDeque<String> = VecDeque::from(vec!["a".into(), "b".into()]);
    let mut cursor: usize = 1;
    apply_immediate_routing_at(&mut q, "b", &mut cursor);
    // b is already at position 1 (the cursor) — no change, cursor advances.
    assert_eq!(q, VecDeque::from(vec!["a".to_string(), "b".into()]));
    assert_eq!(cursor, 2);
}

#[test]
fn immediate_at_empty_queue() {
    let mut q: VecDeque<String> = VecDeque::new();
    let mut cursor: usize = 0;
    apply_immediate_routing_at(&mut q, "opus", &mut cursor);
    assert_eq!(q, VecDeque::from(vec!["opus".to_string()]));
    assert_eq!(cursor, 1);
}

#[test]
fn immediate_at_cursor_clamped_to_len() {
    // cursor beyond queue length should be clamped.
    let mut q: VecDeque<String> = VecDeque::from(vec!["a".into()]);
    let mut cursor: usize = 99;
    apply_immediate_routing_at(&mut q, "b", &mut cursor);
    assert_eq!(q, VecDeque::from(vec!["a".to_string(), "b".into()]));
    assert_eq!(cursor, 2);
}

#[test]
fn immediate_at_full_chain_scenario() {
    // Full scenario: C→@a@b, A→@c, B→(none), C→@a
    let mut q: VecDeque<String> = VecDeque::new();
    let mut cursor: usize = 0;

    // Round 1: C mentions @a @b
    apply_immediate_routing_at(&mut q, "a", &mut cursor);
    apply_immediate_routing_at(&mut q, "b", &mut cursor);
    assert_eq!(q, VecDeque::from(vec!["a".to_string(), "b".into()]));
    assert_eq!(cursor, 2);

    // A pops
    q.pop_front();
    cursor = cursor.saturating_sub(1); // cursor=1
    assert_eq!(q, VecDeque::from(vec!["b".to_string()]));

    // Round 2: A mentions @c — should go after b
    apply_immediate_routing_at(&mut q, "c", &mut cursor);
    assert_eq!(q, VecDeque::from(vec!["b".to_string(), "c".into()]));
    assert_eq!(cursor, 2);

    // B pops
    q.pop_front();
    cursor = cursor.saturating_sub(1); // cursor=1
    assert_eq!(q, VecDeque::from(vec!["c".to_string()]));

    // Round 3: B has no @mention, C runs next
    // C pops
    q.pop_front();
    cursor = cursor.saturating_sub(1); // cursor=0
    assert!(q.is_empty());

    // Round 4: C mentions @a — back to fresh insert at 0
    apply_immediate_routing_at(&mut q, "a", &mut cursor);
    assert_eq!(q, VecDeque::from(vec!["a".to_string()]));
    assert_eq!(cursor, 1);
}

#[test]
fn immediate_at_target_before_cursor_moved() {
    // Target is before cursor — remove and re-insert at adjusted position.
    // Queue: [x, a, b], cursor=3. Move "x" to cursor position.
    let mut q: VecDeque<String> =
        VecDeque::from(vec!["x".into(), "a".into(), "b".into()]);
    let mut cursor: usize = 3;
    apply_immediate_routing_at(&mut q, "x", &mut cursor);
    // "x" was at 0 (before cursor=3), removed → cursor adjusts to 2, insert at 2.
    assert_eq!(
        q,
        VecDeque::from(vec!["a".to_string(), "b".into(), "x".into()])
    );
    assert_eq!(cursor, 3);
}

#[test]
fn immediate_at_with_existing_non_a2a_items() {
    // Original dispatch items [d, e] already in queue, a2a inserts at cursor=0.
    let mut q: VecDeque<String> = VecDeque::from(vec!["d".into(), "e".into()]);
    let mut cursor: usize = 0;
    apply_immediate_routing_at(&mut q, "a", &mut cursor);
    apply_immediate_routing_at(&mut q, "b", &mut cursor);
    // a2a items go before original dispatch items.
    assert_eq!(
        q,
        VecDeque::from(vec![
            "a".to_string(),
            "b".into(),
            "d".into(),
            "e".into()
        ])
    );
    assert_eq!(cursor, 2);
}

#[test]
fn immediate_at_duplicate_in_multi_mention() {
    // Agent mentions @a @b @a — parse_agent_mentions deduplicates, so only a, b arrive.
    // Verify cursor-based routing handles them correctly.
    let mut q: VecDeque<String> = VecDeque::new();
    let mut cursor: usize = 0;
    // Simulating deduplicated result: [a, b]
    apply_immediate_routing_at(&mut q, "a", &mut cursor);
    apply_immediate_routing_at(&mut q, "b", &mut cursor);
    assert_eq!(q, VecDeque::from(vec!["a".to_string(), "b".into()]));
    assert_eq!(cursor, 2);
}

// ── apply_queued_routing tests (multi-target) ───────────────────────

#[test]
fn queued_multi_targets() {
    let mut q: VecDeque<String> = VecDeque::new();
    apply_queued_routing(&mut q, "a");
    apply_queued_routing(&mut q, "b");
    apply_queued_routing(&mut q, "c");
    assert_eq!(
        q,
        VecDeque::from(vec!["a".to_string(), "b".into(), "c".into()])
    );
}

#[test]
fn queued_multi_targets_with_existing() {
    let mut q: VecDeque<String> = VecDeque::from(vec!["x".into()]);
    apply_queued_routing(&mut q, "a");
    apply_queued_routing(&mut q, "b");
    // "x" stays, new targets appended.
    assert_eq!(
        q,
        VecDeque::from(vec!["x".to_string(), "a".into(), "b".into()])
    );
}

#[test]
fn queued_multi_targets_partial_duplicate() {
    let mut q: VecDeque<String> = VecDeque::from(vec!["a".into()]);
    apply_queued_routing(&mut q, "a"); // already in queue — no-op
    apply_queued_routing(&mut q, "b");
    assert_eq!(q, VecDeque::from(vec!["a".to_string(), "b".into()]));
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
