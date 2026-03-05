## Context

Conversations accumulate messages over time, eventually hitting LLM context window limits. The system already tracks per-message `Usage` data and has a stub `/compact` command. The `auto_compact_threshold` config field exists but is not wired up.

Current message flow: user input → `parse_input()` → Agent Loop (LLM call + tool rounds) → `AgentEvent::Done { usage }` → TUI renders + session persists. Token usage is stored per-message in `MessageEntry.usage` and aggregated in `App.agent_token_usage` HashMap.

## Goals / Non-Goals

**Goals:**
- Manual `/compact [agent]` command that compresses history via LLM summarization
- Auto-compact triggered by `prompt_tokens >= auto_compact_threshold`
- Pre-compact backup for rollback safety
- Enhanced `/agents` display with token usage
- Configurable `compact_keep_rounds` setting

**Non-Goals:**
- Per-message token display after each reply (removed from scope)
- Token-based cost estimation or billing
- Incremental/partial compression strategies
- Undo/restore command for compact (backup file exists but no CLI restore)

## Decisions

### 1. Compact as a core operation in `krew-core`

The compact logic (building compression prompt, calling LLM, replacing messages) lives in `krew-core` as a reusable function, not in `krew-cli`. This allows both manual `/compact` and auto-compact to share the same code path.

**Function signature concept:**
```rust
pub async fn compact_session(
    agent: &AgentRuntime,
    messages: &[ChatMessage],
    keep_rounds: usize,
    session_id: &str,
    cwd: &Path,
) -> Result<CompactResult>
```

Returns `CompactResult { summary: String, original_count: usize, kept_count: usize, backup_path: PathBuf }`.

### 2. Conversation round detection

A "round" is defined as: one user message + all subsequent non-user messages (assistant replies, tool calls, tool results) until the next user message. Splitting by user messages is simple and deterministic.

### 3. Summary injection as user message

The compressed summary is injected as a `ChatMessage { role: User, content: "[Session History Summary]\n{summary}" }` at index 0 of the message list. This ensures all agents see it in their context. Using User role avoids complications with system prompt construction.

### 4. Compression prompt (English)

```
Compress the following conversation history into a concise summary. Preserve:
- Key decisions and conclusions
- Important context and constraints
- Action items and their status
- Technical details that would be needed for continuation

Be concise but comprehensive. The summary will replace the original messages.
```

The conversation history is serialized as the user content, and the compression prompt as system prompt.

### 5. Auto-compact trigger point

Check `usage.prompt_tokens >= threshold` after each `AgentEvent::Done`. Set a flag `needs_auto_compact = true`. Before the next user message is processed (in the main event loop), execute compact if flagged. This avoids interrupting an ongoing multi-agent reply.

### 6. Backup file naming

Format: `{session_id}.pre-compact.{unix_timestamp}.toml` in the same `.krew/sessions/` directory. Uses the existing `SessionFile` serialization.

## Risks / Trade-offs

- **LLM summary quality** → Mitigated by keeping last N rounds intact; only older messages are compressed
- **Compression cost** → The compact call itself uses tokens; acceptable trade-off for context reclamation
- **Auto-compact interruption** → Runs before next user message, not during; user sees status message
- **Backup file accumulation** → No auto-cleanup; users manage manually. Low risk since compaction is infrequent
