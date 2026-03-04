## 1. Dependencies & Config

- [x] 1.1 Add workspace dependencies: `similar`, `diffy`, `supports-color` to root `Cargo.toml` `[workspace.dependencies]` with `default-features = false`. Check if `syntect` and `two-face` are already available (used in markdown rendering); if not, add them too. Add `unicode-width` if not already present.
- [x] 1.2 Add `ApprovalMode` enum (already exists in krew-config) (`Suggest`, `AutoEdit`, `FullAuto`) to `krew-config`. Add `approval_mode` field to Settings with `Suggest` as default. Support TOML parsing: `"suggest"`, `"auto-edit"`, `"full-auto"`.
  - File: `crates/krew-config/src/lib.rs`

## 2. Core Types — ReviewDecision & ApprovalRequest

- [x] 2.1 Define `ReviewDecision` enum in `krew-core` with variants: `Approved`, `ApprovedForSession`, `Denied`, `Abort`. Derive `Clone`, `Debug`, `Default` (default = `Denied`).
  - File: `crates/krew-core/src/event.rs` (alongside AgentEvent)
- [x] 2.2 Add `AgentEvent::ApprovalRequest` variant: `{ tool_name: String, arguments: String, diff: Option<String>, respond: tokio::sync::oneshot::Sender<ReviewDecision> }`. This is the mechanism for agent loop to block and await TUI approval.
  - File: `crates/krew-core/src/event.rs`

## 3. Write Tools Implementation

- [x] 3.1 Implement `WriteFileTool` in `crates/krew-tools/src/builtin/write_file.rs`. Follow existing `ReadFileTool` pattern. Use `validate_path()` for boundary check. Create parent dirs with `tokio::fs::create_dir_all`. `requires_approval()` returns `true`. Return file size and line count in result summary.
  - Reference pattern: `crates/krew-tools/src/builtin/read_file.rs`
- [x] 3.2 Implement `EditFileTool` in `crates/krew-tools/src/builtin/edit_file.rs`. Parameters: `file_path`, `old_string`, `new_string`. Read file → verify `old_string` appears exactly once → replace → write back. Use `similar::TextDiff` to generate unified diff string included in ToolResult. `requires_approval()` returns `true`.
  - Reference pattern: `crates/krew-tools/src/builtin/read_file.rs`
  - Diff generation: use `similar::TextDiff::from_lines(old, new).unified_diff().to_string()`

## 4. Shell Tool Implementation

- [x] 4.1 Implement shell detection module: `find_shell()` function that returns `(PathBuf, &str)` (shell path, flag). **Replicate Claude Code's Git Bash detection logic on Windows** — no extra crates. Detection order on Windows: `KREW_BASH_PATH` env → PATH search (skip System32 WSL bash) → `C:\Program Files\Git\bin\bash.exe` → `C:\Program Files (x86)\Git\bin\bash.exe` → error. On Unix: `KREW_BASH_PATH` → `$SHELL` → `/bin/sh`. Cache result with `OnceLock`.
  - File: `crates/krew-tools/src/builtin/shell.rs`
- [x] 4.2 Implement `ShellTool` in `crates/krew-tools/src/builtin/shell.rs`. Use `tokio::process::Command::new(shell).arg(flag).arg(command).current_dir(cwd)`. On Windows, set `CREATE_NO_WINDOW` (0x08000000) via `CommandExt::creation_flags()`. Use `tokio::time::timeout()` with `timeout_seconds` parameter (default 120). Capture stdout+stderr combined. Truncate output at 100KB. `requires_approval()` returns `true`.
  - Reference: Claude Code's shell execution approach

## 5. Tool Registry Update

- [x] 5.1 Add `create_full_registry(cwd: PathBuf) -> ToolRegistry` factory in `crates/krew-tools/src/builtin/mod.rs`. Registers all 6 tools: read_file, glob, grep, write_file, edit_file, shell.
- [x] 5.2 Add `requires_approval(&self, name: &str) -> bool` method to `ToolRegistry`. Delegates to the handler's `requires_approval()`.
  - File: `crates/krew-tools/src/lib.rs`
- [x] 5.3 Update `AgentRuntime` in `krew-core` to use `create_full_registry()` instead of `create_readonly_registry()`.
  - File: `crates/krew-core/src/agent.rs`

## 6. Diff Rendering — Port from Codex

> **照搬 Codex 源码**，适配我们的 inline viewport 架构。

- [x] 6.1 Port terminal color detection module. Create `crates/krew-cli/src/render/terminal_palette.rs`. **照搬** `codex-rs/tui/src/terminal_palette.rs` 中的 `StdoutColorLevel` 枚举、`stdout_color_level()` 函数、`default_bg()` 函数、`XTERM_COLORS` 常量表、`indexed_color()`、`rgb_color()` 函数。去掉 Codex 特有的 `supports-color` 版本差异处理，直接用 `crossterm` 查询终端能力。
  - **照搬**: `../codex/codex-rs/tui/src/terminal_palette.rs`
- [x] 6.2 Port color math module. Create `crates/krew-cli/src/render/color.rs`. **照搬** `codex-rs/tui/src/color.rs` 中的 `is_light()` 函数和 `perceptual_distance()` 函数。这是纯数学计算，无外部依赖，可以原样搬。
  - **照搬**: `../codex/codex-rs/tui/src/color.rs`
- [x] 6.3 Port syntax highlighting for diffs (integrated into diff_render.rs). Create `crates/krew-cli/src/render/highlight.rs`. **照搬** `codex-rs/tui/src/render/highlight.rs` 中的 `highlight_code_to_styled_spans()` 函数、`DiffScopeBackgroundRgbs` 结构体、`diff_scope_background_rgbs()` 函数、`exceeds_highlight_limits()` 函数。需要 `syntect` + `two-face` 依赖。
  - **照搬**: `../codex/codex-rs/tui/src/render/highlight.rs`
- [x] 6.4 Port diff rendering core. Create `crates/krew-cli/src/render/diff_render.rs`. **照搬** `codex-rs/tui/src/diff_render.rs` 中的核心函数和类型:
  - 类型: `DiffLineType`, `DiffTheme`, `DiffColorLevel`, `RichDiffColorLevel`, `ResolvedDiffBackgrounds`, `DiffRenderStyleContext`
  - 公共 API: `current_diff_render_style_context()`, `push_wrapped_diff_line_with_style_context()`, `push_wrapped_diff_line_with_syntax_and_style_context()`, `line_number_width()`, `display_path_for()`, `calculate_add_remove_from_diff()`
  - 内部函数: `diff_theme()`, `diff_color_level()`, `resolve_diff_backgrounds()`, 所有 `style_*` 辅助函数, `wrap_styled_spans()`
  - **删除**: `Renderable` trait 实现, `DiffSummary` 结构体, `render_change()` 函数（我们用自己的方式消费 `Vec<RtLine>`）, `codex_core::git_info` 依赖
  - **替换**: `crate::exec_command::relativize_to_home()` → 我们自己的路径辅助
  - **照搬**: `../codex/codex-rs/tui/src/diff_render.rs`
- [x] 6.5 Port `line_utils.rs` helper. Create `crates/krew-cli/src/render/line_utils.rs`. **照搬** `codex-rs/tui/src/render/line_utils.rs` 中的 `prefix_lines()` 函数。
  - **照搬**: `../codex/codex-rs/tui/src/render/line_utils.rs`
- [x] 6.6 Create diff rendering integration function (render_unified_diff in diff_render.rs). Write a `render_diff_lines(old_content: &str, new_content: &str, file_path: &str, width: usize) -> Vec<RtLine<'static>>` function that: parses unified diff with `diffy` → calls `push_wrapped_diff_line_with_syntax_and_style_context()` for each hunk line → returns colored lines ready for `insert_widget_above()`.
  - File: `crates/krew-cli/src/render/diff_render.rs` (new public function)

## 7. Approval TUI — Port from Codex

> **照搬 Codex 审批 overlay**，精简掉 MCP/网络/多线程/ExecPolicy 等我们不需要的功能。

- [x] 7.1 Port `ListSelectionView` widget (integrated into ApprovalOverlay). Create `crates/krew-cli/src/app/approval/list_selection.rs`. **照搬** `codex-rs/tui/src/bottom_pane/list_selection_view.rs` 的核心逻辑:
  - 保留: `SelectionItem` 结构体 (简化字段: name, display_shortcut, is_current), `ListSelectionView` 结构体, 键盘处理 (↑↓ Enter Esc 快捷键), 渲染 (行高亮、快捷键标签)
  - 删除: 搜索/过滤功能, side content 面板, `SideContentWidth`, `OnSelectionChangedCallback`, `OnCancelCallback`
  - **照搬**: `../codex/codex-rs/tui/src/bottom_pane/list_selection_view.rs`
- [x] 7.2 Port selection popup rendering (integrated into ApprovalOverlay). Create `crates/krew-cli/src/app/approval/popup_render.rs`. **照搬** `codex-rs/tui/src/bottom_pane/selection_popup_common.rs` 的渲染核心:
  - 保留: `GenericDisplayRow` 结构体, `render_menu_surface()`, `render_rows()`, `apply_row_state_style()`, `build_full_line()`
  - 删除: `ColumnWidthMode::AutoAllRows` 和 `Fixed`, fuzzy match 高亮, 复杂两列自动宽度计算
  - **照搬**: `../codex/codex-rs/tui/src/bottom_pane/selection_popup_common.rs`
- [x] 7.3 Implement `ApprovalOverlay` widget. Create `crates/krew-cli/src/app/approval/overlay.rs`. **参考** `codex-rs/tui/src/bottom_pane/approval_overlay.rs` 但大幅简化:
  - 保留: `ApprovalRequest` 枚举 (仅 Exec 和 Patch 两个变体), 选项构建 (y/a/n/esc 快捷键), 队列管理 (`enqueue_request`, `advance_queue`), Ctrl+C 处理
  - 删除: MCP elicitation, 网络审批, ExecPolicy amendment, 多线程标签, `FullScreenApprovalRequest`, `SelectAgentThread`
  - 适配: 用 `oneshot::Sender<ReviewDecision>` 代替 Codex 的 `AppEvent::SubmitThreadOp(Op::ExecApproval{...})`
  - Header 构建: Shell → 显示命令; Edit/Write → 显示 diff (用 6.6 的 `render_diff_lines()`)
  - **照搬**: `../codex/codex-rs/tui/src/bottom_pane/approval_overlay.rs`
- [x] 7.4 Create `crates/krew-cli/src/app/approval/mod.rs` module file, re-export `ApprovalOverlay`, `ListSelectionView`, and related types.

## 8. Agent Loop Approval Integration

- [x] 8.1 Implement approval session cache in agent loop. Store `HashMap<String, ReviewDecision>` keyed by `format!("{}:{}", tool_name, arguments)`. Check cache before sending ApprovalRequest. On `ApprovedForSession`, insert into cache.
  - **参考**: `../codex/codex-rs/core/src/tools/sandboxing.rs` 中的 `ApprovalStore` 和 `with_cached_approval()` 模式
  - File: `crates/krew-core/src/agent.rs`
- [x] 8.2 Modify agent loop tool execution flow. Before executing each tool: (1) check `requires_approval()`, (2) evaluate `ApprovalMode`, (3) check cache, (4) if approval needed: send `AgentEvent::ApprovalRequest` → await oneshot → handle decision. Separate tools into auto-execute group and approval-needed group. Auto-execute group runs in parallel; approval-needed group runs sequentially (each waits for user decision).
  - File: `crates/krew-core/src/agent.rs`
- [x] 8.3 Handle `ReviewDecision::Denied` — return `ToolResult { content: "User denied ...", is_error: true }`. Handle `ReviewDecision::Abort` — break out of tool loop, send `AgentEvent::Error`.
  - File: `crates/krew-core/src/agent.rs`

## 9. TUI Event Integration

- [x] 9.1 Handle `AgentEvent::ApprovalRequest` in TUI event loop. When received: create `ApprovalOverlay` instance, push it onto a view stack or replace current popup. The overlay holds the `oneshot::Sender` and sends the decision when user selects an option.
  - File: `crates/krew-cli/src/app/state.rs` (`handle_agent_event()` method)
- [x] 9.2 Integrate approval overlay rendering into viewport. The overlay renders in the input viewport area (above the separator). When overlay is active, input is disabled. When overlay dismisses, input is re-enabled.
  - File: `crates/krew-cli/src/render/viewport.rs`
- [x] 9.3 Route keyboard events to approval overlay when active. When overlay is present, key events go to overlay first. Only if overlay is `is_complete()` do events pass to input area.
  - File: `crates/krew-cli/src/app/state.rs` (`handle_event()` method)

## 10. Tool Call Rendering Updates

- [x] 10.1 Update tool call rendering for write tools. When `ToolCallDone` arrives for edit_file, render the diff using diff rendering module (colored lines). For write_file, render a brief content summary. For shell, render command output preserving raw format.
  - File: `crates/krew-cli/src/app/state.rs` (`handle_agent_event()` / agent display helpers)
- [x] 10.2 Update `insert_tool_line()` (diff preview via ApprovalRequest.diff) or create new rendering helpers for diff output. Use `insert_widget_above()` with a `Paragraph` widget containing the colored `Vec<RtLine>` from the diff renderer.
  - File: `crates/krew-cli/src/app/agent_display.rs` or equivalent

## 11. Testing & Verification

- [x] 11.1 Unit tests for write_file: boundary check, create parents, overwrite.
- [x] 11.2 Unit tests for edit_file: single match, no match, multiple matches (error), diff generation.
- [x] 11.3 Unit tests for shell: timeout, output truncation, exit code.
- [x] 11.4 Unit tests for approval policy evaluation: all 3 modes × 3 tool types matrix.
- [x] 11.5 Integration test: `cargo build` compiles, `cargo clippy` passes, `cargo fmt --check` passes.
- [x] 11.6 Manual test: run the CLI, ask agent to create a file → approval prompt appears → press y → file created. Ask agent to edit a file → diff shown → approve → file modified. Ask agent to run `cargo test` → shell approval → approve → output shown.
