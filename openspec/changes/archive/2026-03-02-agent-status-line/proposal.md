## Why

Agent 流式回复期间，用户无法直观感知 agent 当前的工作状态——不知道哪个 agent 在工作、已经过了多久、能否中断。主流 AI CLI 工具（如 Claude Code）都在输入框上方提供实时状态指示器（spinner 动画 + 计时器 + 中断提示），这已成为用户预期的标准体验。

## What Changes

- 新增一行**动态 agent 状态行**，在 agent 处理期间显示于输入区域上方分隔线的正上方
- 状态行包含：animated spinner（`●`/`◦` 闪烁）、agent 名称、"Working" 文字、已用时间、ESC 中断提示
- 状态行仅在 agent 活跃时出现（viewport 从 4 行动态扩展到 5 行），空闲时恢复 4 行布局
- 格式示例：`● claude Working  (12s · ESC to interrupt)`

## Capabilities

### New Capabilities
- `agent-status-indicator`: Agent 工作状态指示器——spinner 动画、计时器、中断提示的渲染与生命周期管理

### Modified Capabilities
- `tui-framework`: 输入区域布局需支持动态状态行高度，viewport 高度计算逻辑调整
- `agent-display`: Agent 事件处理需驱动状态指示器的显示/隐藏和计时器启停

## Impact

- **代码影响**：`crates/krew-cli/src/render/viewport.rs`（布局计算与渲染）、`crates/krew-cli/src/app/state.rs`（状态管理与事件循环）、`crates/krew-cli/src/app/agent_display.rs`（事件驱动）
- **依赖**：无新增依赖，复用已有的 ratatui + crossterm + tokio Instant
- **向后兼容**：纯 UI 增强，不影响现有功能和 API
