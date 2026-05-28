---
kind: code-report
change: anthropic-thinking-blocks
workflow: code-refine
type: standard
status: approved
rounds_completed: 4
reviewer: codex
target_source: default-from-change
session_ids:
  correctness: 019e6f47-dc34-7672-9f24-5c058d62c045
  failure-modes: 019e6f47-dce7-7b63-966b-3c67362982ae
  readability: 019e6f47-dd69-7681-be33-cc8277f7ad34
  test-alignment: 019e6f47-dcc4-79b1-8240-76ae505a8a4b
started_at: 2026-05-28T15:48:24Z
updated_at: 2026-05-28T16:24:53Z
---

## Round 1

### Verdict
NEEDS-ATTENTION

### Findings

#### Perspective: correctness

#### Finding 1: Prompt mode drops final-turn thinking blocks before persistence
- Severity: major
- Type: [Actionable]
- Location: crates/krew-cli/src/prompt_mode/mod.rs:327, :523, :240
- Description: `AgentEvent::Done.final_thinking_blocks` is intentionally propagated by the agent loop, and TUI mode attaches it to the final assistant message before saving. Prompt mode currently destructures it as `_`, never stores it in `AgentResult`, and then persists a final `ChatMessage` with the default empty `thinking_blocks`. Any `krew -p` Claude run that ends with a thinking final answer saves a session missing the final assistant thinking blocks, violating the design requirement that terminal assistant messages retain `StreamResult.thinking_blocks`.
- Suggestion: Add `final_thinking_blocks: Vec<krew_llm::ThinkingBlock>` to `AgentResult`, collect it in the `AgentEvent::Done` arm, return it from `consume_agent_events`, and assign `final_msg.thinking_blocks = result.final_thinking_blocks;` before pushing.

Full text: tmp/anthropic-thinking-blocks/xd-spec-code-refine-output-1-1.md

#### Perspective: failure-modes

#### Finding 1: Anthropic SSE persists invalid incomplete thinking blocks
- Severity: major
- Type: [Actionable]
- Location: crates/krew-llm/src/anthropic.rs:673 (content_block_stop)
- Description: `content_block_stop` emits `ThinkingBlockDone` even when a `thinking` block never received `signature_delta`, and emits `Redacted { data: "" }` when `redacted_thinking` lacks `data`. Those blocks are illegal replay history and can be persisted, causing the next Anthropic/Vertex request to fail with the same class of HTTP 400 this change is meant to prevent.
- Suggestion: Validate `state.thinking_signature` (non-empty) and `state.redacted_data` (Some) in the stop branches; on missing required fields, clear block state and emit `StreamEvent::Error(...)`. Add malformed-stream tests.

#### Finding 2: Prompt mode drops final thinking blocks before session persistence
- Severity: major
- Type: [Actionable]
- Location: crates/krew-cli/src/prompt_mode/mod.rs:523
- Description: Duplicate of correctness #1 from the failure-mode angle: `consume_agent_events` destructures `AgentEvent::Done` with `final_thinking_blocks: _`, and `AgentResult` has no field for it. Unlike TUI (`app/state.rs:877`) and task mode (`task/mod.rs:147`), `krew -p` writes resumable history missing the terminal assistant thinking blocks.
- Suggestion: Thread `final_thinking_blocks` through `AgentResult`; add a prompt-mode test that sends `Done { final_thinking_blocks: vec![...] }` and asserts the returned `AgentResult` preserves the block.

Full text: tmp/anthropic-thinking-blocks/xd-spec-code-refine-output-1-2.md

#### Perspective: readability

#### Finding 1: Prompt mode hides and drops final thinking blocks
- Severity: major
- Type: [Actionable]
- Location: crates/krew-cli/src/prompt_mode/mod.rs:528
- Description: Same root cause as correctness #1 / failure-modes #2 from the readability angle: `final_thinking_blocks: _` reads as if thinking is display-only noise, but it is protocol replay state. The persistence boundary becomes hard to reason about because TUI/task carry it while prompt drops it silently.
- Suggestion: Thread `final_thinking_blocks` through `AgentResult` and assign on `final_msg`.

#### Finding 2: Anthropic thinking wire strings should be named constants
- Severity: minor
- Type: [Actionable]
- Location: crates/krew-llm/src/anthropic.rs:271/276 (serialiser), :547/553 (state machine), :604/626 (delta dispatch), :673/685 (stop dispatch)
- Description: Production code repeats Anthropic protocol strings (`"thinking"`, `"redacted_thinking"`, `"thinking_delta"`, `"signature_delta"`) across request serialisation and SSE parsing; easy to mistype, and the comparisons read less self-documenting than surrounding named helpers.
- Suggestion: Add `BLOCK_TYPE_THINKING`, `BLOCK_TYPE_REDACTED_THINKING`, `DELTA_TYPE_THINKING`, `DELTA_TYPE_SIGNATURE` constants near `ANTHROPIC_VERSION`; use them in production code, leave test literals as wire-format assertions.

Full text: tmp/anthropic-thinking-blocks/xd-spec-code-refine-output-1-3.md

#### Perspective: test-alignment

| # | Severity | Type         | Suggestion                                                                                       |
|---|----------|--------------|--------------------------------------------------------------------------------------------------|
| 1 | major    | [Actionable] | Thread `final_thinking_blocks` through prompt mode + add a `Done`-event propagation test.        |
| 2 | minor    | [Actionable] | Replace `Option::is_none` with empty-aware predicate so `Some(vec![])` also omits the TOML key.  |
| 3 | minor    | [Actionable] | `CannedClient` records each call's `messages`; assert round-2 input replays round-1 thinking.    |

Full text: tmp/anthropic-thinking-blocks/xd-spec-code-refine-output-1-4.md

### Actions Taken
- crates/krew-cli/src/prompt_mode/mod.rs — added `final_thinking_blocks` field to `AgentResult`, threaded through `consume_agent_events`, attached to final assistant message before `messages.push` (by correctness, failure-modes, readability, test-alignment).
- crates/krew-cli/src/prompt_mode/tests.rs — added `final_thinking_blocks_propagate_from_done_to_agent_result` (by test-alignment).
- crates/krew-llm/src/anthropic.rs — SSE `content_block_stop` now validates `signature` (non-empty) and `data` (Some) before emitting `ThinkingBlockDone`, surfaces `StreamEvent::Error` otherwise; introduced `BLOCK_TYPE_THINKING` / `BLOCK_TYPE_REDACTED_THINKING` / `DELTA_TYPE_THINKING` / `DELTA_TYPE_SIGNATURE` constants and used them in `thinking_blocks_to_json` + state machine; added `sse_thinking_block_without_signature_emits_error_no_done` and `sse_redacted_thinking_block_without_data_emits_error_no_done` (by failure-modes, readability).
- crates/krew-storage/src/session_file.rs — replaced `skip_serializing_if = "Option::is_none"` on `thinking_blocks` with `thinking_blocks_is_absent_or_empty` so `Some(vec![])` also omits the TOML key (by test-alignment).
- crates/krew-storage/tests/session_file_test.rs — added `test_empty_thinking_blocks_omits_key_when_some_empty_vec` (by test-alignment).
- crates/krew-core/src/agent/agent_loop.rs — `CannedClient` now records each `chat_stream`'s `messages.to_vec()`; `agent_loop_attaches_thinking_blocks_to_each_round` additionally asserts the round-2 LLM input contains the round-1 assistant message with replayed `thinking_blocks` (by test-alignment).

## Round 2

### Verdict
NEEDS-ATTENTION

### Findings

#### Perspective: correctness

#### Finding 1: Fatal malformed-thinking errors do not terminate the Anthropic stream
- Severity: major
- Type: [Actionable]
- Location: crates/krew-llm/src/anthropic.rs:689, :715
- Description: The Round 1 `content_block_stop` validation emits `StreamEvent::Error` when the thinking signature or redacted data is missing, but returns with `done = false`. A direct `chat_stream` consumer that keeps polling can still receive subsequent events from the same malformed SSE stream (including `message_stop` as `StreamEvent::Done`). The agent loop happens to return on first `Error`, but the provider stream itself remains non-terminal, violating the intended fatal-error semantics.
- Suggestion: Set `done = true` before returning each malformed-thinking error; strengthen the malformed-stream tests to assert no `Done(_)` is observed after the error.

Full text: tmp/anthropic-thinking-blocks/xd-spec-code-refine-output-2-1.md

#### Perspective: failure-modes

#### Finding 1: Malformed thinking-block errors are non-terminal
- Severity: minor
- Type: [Actionable]
- Location: crates/krew-llm/src/anthropic.rs:689
- Description: Same root cause as correctness #1 from the failure-mode angle — the missing-signature / missing-data branches leave `done = false`, so downstream consumers could observe contradictory terminal state.
- Suggestion: Before each new malformed-block error return, set `done = true`; add assertions in both malformed-stream tests that no `Done(_)` is emitted after the error.

Full text: tmp/anthropic-thinking-blocks/xd-spec-code-refine-output-2-2.md

#### Perspective: readability
(none)

#### Perspective: test-alignment

#### Finding 1: Prompt-mode final message push is still untested
- Severity: minor
- Type: [Actionable]
- Location: crates/krew-cli/src/prompt_mode/mod.rs:240
- Description: The Round 1 `final_thinking_blocks_propagate_from_done_to_agent_result` test only covers `AgentResult` plumbing. Removing `final_msg.thinking_blocks = result.final_thinking_blocks;` would not break any prompt-mode test, so the persistence path is test-aligned only by inspection.
- Suggestion: Extract the final assistant-message construction into a small helper used by `run_prompt_mode`, then unit-test that helper.

#### Finding 2: Redacted TOML shape is not asserted
- Severity: minor
- Type: [Actionable]
- Location: crates/krew-storage/tests/session_file_test.rs:291
- Description: The round-trip test verifies a `RedactedThinking` entry reloads correctly, but does not assert the on-disk TOML shape required by the spec: the redacted block must carry the `block_type = "redacted_thinking"` tag with `data` only — no `text` or `signature` keys leaking from the `Thinking` variant.
- Suggestion: After `save_session`, read the TOML and assert the redacted section contains `block_type = "redacted_thinking"` + `data = "opaque"`, and does NOT contain `text =` / `signature =`.

Full text: tmp/anthropic-thinking-blocks/xd-spec-code-refine-output-2-4.md

### Actions Taken
- crates/krew-llm/src/anthropic.rs — set `done = true` before both new malformed-thinking `StreamEvent::Error` returns; extended `sse_thinking_block_without_signature_emits_error_no_done` and `sse_redacted_thinking_block_without_data_emits_error_no_done` to additionally assert no `Done(_)` follows the error (by correctness, failure-modes).
- crates/krew-cli/src/prompt_mode/mod.rs — extracted `build_final_assistant_msg` helper; `run_prompt_mode` calls it with `&mut result` and `intermediate_messages` is now taken via `std::mem::take` so the partial-move/borrow pattern stays clean (by test-alignment).
- crates/krew-cli/src/prompt_mode/tests.rs — added `build_final_assistant_msg_attaches_thinking_blocks_and_metadata` that constructs a full `AgentResult` (thinking + redacted blocks, server tools, usage, whisper targets) and asserts the produced `ChatMessage` carries every field plus that the helper took the heavy fields out of `AgentResult` (by test-alignment).
- crates/krew-storage/tests/session_file_test.rs — `test_thinking_blocks_roundtrip_with_both_variants` now reads the saved TOML and asserts the redacted block carries `data = "opaque"` and contains neither `text =` nor `signature =` (by test-alignment).

## Round 3

### Verdict
NEEDS-ATTENTION

### Findings

#### Perspective: correctness

#### Finding 1: Full stale-tool pruning drops preserved thinking blocks
- Severity: minor
- Type: [Actionable]
- Location: crates/krew-core/src/agent/prune.rs:198
- Description: When every tool call on an assistant message is stale but the assistant has non-empty text, `prune_stale_tool_calls` rebuilds the message with `ChatMessage::text(...)`, which resets `thinking_blocks` to an empty vec. A current-agent Anthropic message with thinking + text + a now-stale tool call will be replayed without its valid thinking signature after `prepare_messages_for_agent` prunes it. The partial-prune branch already preserves `thinking_blocks`; the all-stale-with-text branch should too.
- Suggestion: Clear `tool_calls` on the original message instead of going through `ChatMessage::text`; add a regression test.

Full text: tmp/anthropic-thinking-blocks/xd-spec-code-refine-output-3-1.md

#### Perspective: failure-modes
(none)

#### Perspective: readability

#### Finding 1: Non-English comment text remains in target code
- Severity: nit
- Type: [Actionable]
- Location: crates/krew-llm/src/openai_chat.rs:300
- Description: `build_compatible_thinking_fields` doc still contains Chinese words (`官方`, `百炼`), violating the repository's English-comments convention.
- Suggestion: Rewrite the two bullets in English.

Full text: tmp/anthropic-thinking-blocks/xd-spec-code-refine-output-3-3.md

#### Perspective: test-alignment
(none)

### Actions Taken
- crates/krew-core/src/agent/prune.rs — all-stale-with-text branch now clears `tool_calls` on the original message instead of rebuilding via `ChatMessage::text`, preserving `thinking_blocks` (and other metadata) for replay (by correctness).
- crates/krew-core/src/agent/prune.rs — added `prune_all_stale_with_text_preserves_thinking_blocks` regression test that fails if the rebuild path silently drops thinking blocks (by correctness).
- crates/krew-llm/src/openai_chat.rs — translated the two Chinese comment fragments in `build_compatible_thinking_fields` doc to English (by readability).

## Round 4

### Verdict
APPROVE

### Findings

#### Perspective: correctness
(none)

#### Perspective: failure-modes
(none)

#### Perspective: readability
(none)

#### Perspective: test-alignment
(none)

### Actions Taken
(none)
