## Context

Phase 8 established the readonly tool system: `ToolHandler` trait, `ToolRegistry`, agent loop tool-call cycle, and 3 readonly tools. The agent loop currently executes all tools in parallel without any approval check.

We have the Codex CLI source code at `../codex` which contains production-grade implementations of:
- Diff rendering with syntax highlighting and theme awareness (`codex-rs/tui/src/diff_render.rs`)
- Approval overlay UI (`codex-rs/tui/src/bottom_pane/approval_overlay.rs`)
- List selection widget (`codex-rs/tui/src/bottom_pane/list_selection_view.rs`)
- Approval caching (`codex-rs/core/src/tools/sandboxing.rs`)

We also reference Claude Code's approach to Git Bash detection on Windows.

## Goals / Non-Goals

**Goals:**
- Implement write_file, edit_file, and shell tools
- Port Codex's diff rendering for edit operations (GitHub-style, theme-aware, syntax-highlighted)
- Port Codex's approval overlay UI (keyboard shortcuts, queue management)
- Implement approval flow using oneshot channels (agent loop blocks, TUI responds)
- Support three approval modes: suggest, auto-edit, full-auto
- Cross-platform shell detection (Git Bash on Windows, $SHELL on Unix)

**Non-Goals:**
- Sandbox/seccomp execution (Codex-specific, too complex for now)
- MCP elicitation approval (Phase 10)
- Network approval (not applicable)
- ExecPolicy amendment / persistent allow rules (future phase)
- Multi-thread approval routing (we have serial agent loop)
- Diff side-by-side view (vertical unified only)

## Decisions

### D1: Port Codex diff_render.rs for diff display

**Decision**: Port the core diff rendering logic from Codex `diff_render.rs` (2424 lines), adapting the output layer for our inline viewport architecture.

**Rationale**: Codex has a production-grade diff renderer with TrueColor/256/16 support, dark/light theme detection, syntax highlighting, and proper Unicode wrapping. Building from scratch would take significant effort and produce an inferior result.

**What to port**: Core rendering functions (`push_wrapped_diff_line_*`, style helpers, theme detection, color quantization), plus supporting modules (`color.rs`, `terminal_palette.rs`, `render/highlight.rs`).

**What to adapt**: Replace Codex's `Renderable` trait output with `Vec<RtLine<'static>>` consumed by our `insert_widget_above()`. Remove `codex_core::git_info` dependency (use our own path utilities).

**Alternatives considered**: Using `similar` crate's built-in display (too basic, no colors), writing custom renderer (reinventing the wheel).

### D2: Approval flow via oneshot channel embedded in AgentEvent

**Decision**: Add `AgentEvent::ApprovalRequest` variant carrying a `oneshot::Sender<ReviewDecision>`. Agent loop sends this event and awaits the receiver. TUI displays overlay, user decides, TUI sends decision via the oneshot sender.

**Rationale**: This is the simplest approach for our serial agent loop. No need for a separate TUI→Core channel, no need for approval ID routing. The oneshot is self-contained per request.

**Alternatives considered**: Separate mpsc channel TUI→Core (unnecessary complexity), shared mutex state (race-prone), Codex's full Op routing system (overkill for serial loop).

### D3: Shell tool replicates Claude Code's Git Bash detection without extra crates

**Decision**: Implement shell detection using only std + tokio. On Windows: check `KREW_BASH_PATH` env → search PATH (skip System32) → hardcoded Git for Windows paths. On Unix: use `$SHELL` or `/bin/sh`. Set `CREATE_NO_WINDOW` on Windows.

**Rationale**: Claude Code has proven this approach works. The `which` crate adds unnecessary dependency for a simple path search. Timeout as a tool parameter gives the LLM control over long-running commands.

### D4: Port Codex approval overlay with simplifications

**Decision**: Port `approval_overlay.rs` and `list_selection_view.rs`, removing multi-thread support, MCP elicitation, network approval, and exec policy amendments.

**Rationale**: Codex's approval UI has good UX (keyboard shortcuts y/a/n/esc, queue management, visual layout). We keep the core interaction pattern but strip features we don't need yet.

**Simplified ReviewDecision**:
- `Approved` — execute this time
- `ApprovedForSession` — don't ask again this session for this tool+args
- `Denied` — skip execution, tell LLM it was denied
- `Abort` — stop the entire agent turn

### D5: edit_file uses search-replace, not patch grammar

**Decision**: edit_file takes `(file_path, old_string, new_string)` parameters (same as Claude Code's approach), not Codex's Lark grammar patch format.

**Rationale**: Search-replace is simpler for LLMs to use correctly. Generate unified diff via `similar` crate for display. Codex's patch grammar requires a custom parser and is prone to LLM formatting errors.

### D6: Approval policy evaluated in agent loop before tool execution

**Decision**: The agent loop checks `ApprovalMode` (from config) + `tool.requires_approval()` to decide whether to send an `ApprovalRequest` event. Readonly tools always skip. Write tools check mode. Shell always requires approval except in full-auto.

| Tool Type | suggest | auto-edit | full-auto |
|-----------|---------|-----------|-----------|
| read_file, glob, grep | auto | auto | auto |
| write_file, edit_file | approve | auto | auto |
| shell | approve | approve | auto |

### D7: New crate dependencies

**Decision**:
- `similar` — unified diff generation (for edit_file diff preview)
- `diffy` — diff hunk parsing (for rendering Codex-style hunks)
- `syntect` + `two-face` — syntax highlighting in diffs (check if already available via existing markdown rendering; if so, reuse)
- `supports-color` — terminal color level detection

All added to workspace `[workspace.dependencies]` with `default-features = false`.

## Risks / Trade-offs

- **[Risk] Codex code assumes alternate screen** → Mitigation: We only port the line-rendering functions that return `Vec<RtLine>`, not the `Renderable` trait layer. Our inline viewport consumes these lines via `insert_widget_above()`.
- **[Risk] Diff rendering dependencies bloat binary** → Mitigation: `syntect` is the main contributor; check if already pulled in by markdown rendering. `similar` and `diffy` are lightweight.
- **[Risk] Shell timeout kills long builds** → Mitigation: Default 120s, but LLM can pass custom timeout. Document in tool description that `cargo build` etc. may need longer timeout.
- **[Risk] Git Bash not installed on Windows** → Mitigation: Clear error message with installation instructions, plus `KREW_BASH_PATH` env var escape hatch.
- **[Risk] Approval overlay blocks streaming** → Mitigation: The overlay only appears when the agent loop is already paused (awaiting oneshot). No streaming is happening at that point because the LLM call completed and returned tool calls.
