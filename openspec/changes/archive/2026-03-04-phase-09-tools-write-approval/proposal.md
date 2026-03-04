## Why

Phase 8 completed the readonly tool system (read_file, glob, grep). Agents can now read code but cannot modify it. Without write capabilities and shell execution, agents are limited to analysis — they cannot fix bugs, create files, run tests, or execute builds. Phase 9 unlocks the full agent workflow: read → think → write → verify.

Write operations and shell execution are inherently destructive, so they require an approval mechanism to keep the user in control. The Codex CLI (available at `../codex`) has a production-grade implementation of diff rendering and approval UI that we can directly port.

## What Changes

- **write_file tool**: Create or overwrite files within the workspace boundary
- **edit_file tool**: Search-and-replace editing with unified diff preview (port Codex diff rendering)
- **shell tool**: Cross-platform shell execution (replicate Claude Code's Git Bash detection on Windows), timeout as tool parameter (default 120s)
- **Approval flow**: Agent loop pauses for user confirmation on write/shell operations, using oneshot channels for async blocking
- **Approval TUI**: Modal overlay for approve/deny (port from Codex approval_overlay + list_selection_view)
- **Approval policy**: Three modes from config — `suggest` (all write+shell need approval), `auto-edit` (write auto, shell needs approval), `full-auto` (all auto)
- **Diff rendering**: GitHub-style colored diff with syntax highlighting, theme-aware (dark/light), TrueColor/256/16 support (port from Codex diff_render.rs)

## Capabilities

### New Capabilities
- `write-file-tool`: write_file tool implementation with path boundary enforcement
- `edit-file-tool`: edit_file tool with search-replace and unified diff generation
- `shell-tool`: Cross-platform shell execution tool with timeout and output capture
- `tool-approval-flow`: Approval mechanism — oneshot channel blocking, approval policy evaluation, session-scoped approval cache
- `approval-tui`: Approval overlay widget with keyboard shortcuts and diff preview
- `diff-rendering`: Theme-aware diff line rendering with syntax highlighting (ported from Codex)

### Modified Capabilities
- `agent-loop-tool-calls`: Agent loop must check approval policy before executing tools and block on approval channel when needed
- `tool-registry`: Registry must support registering write tools alongside readonly tools
- `tool-rendering`: Tool call TUI rendering must support showing diffs and approval prompts

## Impact

- **Crates modified**: krew-tools (new tools), krew-core (agent loop approval), krew-cli (approval TUI, diff rendering), krew-config (approval mode)
- **New dependencies**: `similar` (unified diff generation), `diffy` (diff parsing), `syntect`/`two-face` (syntax highlighting for diffs — check if already available via markdown rendering)
- **No new crates**: Shell detection uses only std + tokio, no `which` crate
- **Config**: `approval_mode` field in settings.toml (default: `suggest`)
- **Event system**: New `AgentEvent::ApprovalRequest` variant with oneshot channel
