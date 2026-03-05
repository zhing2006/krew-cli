## Why

As conversations grow longer, token costs increase and context windows fill up. Users need a way to compress conversation history to reclaim context space while preserving key information. Additionally, the `/agents` command should display per-agent token usage to help users understand resource consumption.

## What Changes

- Implement `/compact [agent]` command to compress conversation history into a summary
  - Agent parameter optional, defaults to `reply_order[0]`
  - Keeps last N conversation rounds (configurable via `compact_keep_rounds`, default 10)
  - Backs up pre-compact session to `.pre-compact.{timestamp}.toml`
  - Injects compressed summary as a user message at the start of the message list
- Implement auto-compact: when `prompt_tokens >= auto_compact_threshold`, automatically compact before next user message
  - Uses `reply_order[0]` agent for compression
  - Displays status message after compression
- Enhance `/agents` command to show per-agent token usage (prompt_tokens / completion_tokens from last response)
- Add `compact_keep_rounds` setting to `settings.toml`

## Capabilities

### New Capabilities
- `compact`: Manual and automatic conversation history compression with backup and recovery

### Modified Capabilities

## Impact

- `krew-core`: New compact logic (build compression prompt, execute LLM call, replace messages)
- `krew-cli`: `/compact` command execution, auto-compact trigger check, enhanced `/agents` display
- `krew-config`: New `compact_keep_rounds` setting
- `krew-storage`: Backup file creation (`.pre-compact.{timestamp}.toml`)
