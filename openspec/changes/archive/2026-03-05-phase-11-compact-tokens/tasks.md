## 1. Configuration

- [x] 1.1 Add `compact_keep_rounds: usize` field to `Settings` in `krew-config` with default value 10

## 2. Core Compact Logic

- [x] 2.1 Create `compact.rs` module in `krew-core` with `compact_session()` function: split messages into rounds, build compression prompt, call LLM, return `CompactResult`
- [x] 2.2 Implement conversation round detection (split by user messages) and preservation of last N rounds
- [x] 2.3 Implement pre-compact backup: serialize current `SessionFile` to `{id}.pre-compact.{timestamp}.toml`
- [x] 2.4 Implement summary injection as user-role message at index 0 with `[Session History Summary]` prefix

## 3. Manual /compact Command

- [x] 3.1 Wire up `/compact [agent]` command execution in `krew-cli`: resolve agent (default to `reply_order[0]`), validate agent exists, call `compact_session()`, update app state and persist
- [x] 3.2 Display compact result: token reduction message and backup path

## 4. Auto-Compact

- [x] 4.1 Add `needs_auto_compact` flag to app state, set it when `usage.prompt_tokens >= auto_compact_threshold` after `AgentEvent::Done`
- [x] 4.2 Check flag before processing next user message; if set, execute compact using `reply_order[0]` and display status message

## 5. Enhanced /agents Display

- [x] 5.1 Update `/agents` command to show per-agent token usage in format "N tokens (X in / Y out)" with a total line
