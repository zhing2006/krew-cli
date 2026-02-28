## Context

krew-cli 项目已有 Cargo workspace 骨架，包含 6 个 crate。`krew-cli` crate 有 `main.rs`（clap 参数解析）、`app.rs`（App 结构体 + AGENTS.md 加载）和空的 `render.rs`。当前 `cargo run` 仅解析参数后退出，无任何用户界面。

Phase 1 需要将 `krew-cli` crate 从一个空壳变为可交互的全屏 TUI 应用，具备日志、输入和 echo 回显能力。参考了 codex-rs TUI 的实现模式（crossterm raw mode、keyboard enhancement、alternate screen）。

## Goals / Non-Goals

**Goals:**
- 建立 tracing 日志系统，写入 `.krew/logs/` 文件，不干扰 TUI 显示
- 搭建 ratatui 全屏 TUI，包含输出区域和输入框
- 实现多行输入（Shift+Enter 换行、Enter 发送）
- Echo 回显用户输入，验证完整的输入→输出通路
- 支持 `/quit` 和 Ctrl+C 退出

**Non-Goals:**
- 不接入 LLM（Phase 4）
- 不实现 Markdown 渲染（Phase 4）
- 不实现 @ 寻址和 slash 命令系统（Phase 3）——仅硬编码 `/quit`
- 不实现配置文件加载（Phase 2）
- 不实现输入历史（Phase 12）
- 不实现 `@` / `/` 补全（Phase 12）

## Decisions

### D1: TUI 架构采用 ratatui + crossterm

**选择**: 使用 ratatui 的 crossterm backend，参考 codex-rs 的模式。

**理由**: TDD 已选型 ratatui，Cargo.toml 已配置 `features = ["crossterm"]`。crossterm 跨平台支持好，ratatui 是 Rust TUI 生态事实标准。

**替代方案**: 使用 termion backend——但 termion 不支持 Windows。

### D2: 日志使用 tracing + tracing-appender 写入文件

**选择**: 用 `tracing-appender` crate 的 `RollingFileAppender` 写日志到 `.krew/logs/`。

**理由**: tracing 已在 workspace dependencies 中。TUI 应用不能将日志输出到 stdout/stderr（会破坏界面），必须写文件。`tracing-appender` 是 tracing 官方配套 crate，提供非阻塞文件写入。

**替代方案**: 自己实现文件 writer——不必要的重复工作。

### D3: 事件循环采用 tokio select! 模式

**选择**: 主循环用 `tokio::select!` 同时监听 crossterm 事件和内部 channel 消息。

**理由**: tokio 是项目选定的异步运行时。`crossterm::event::EventStream` 提供异步事件流，可以与 tokio select! 配合。这也为后续接入 LLM 流式输出做好准备。

**替代方案**: 同步事件循环（`crossterm::event::read()`）——会阻塞异步任务。

### D4: 输入区域使用 tui-textarea crate

**选择**: 使用 `tui-textarea` crate 处理多行输入编辑。

**理由**: 多行输入需要光标移动、文本插入删除、换行等功能。`tui-textarea` 是 ratatui 生态中成熟的文本输入组件，支持多行编辑、剪贴板等，避免从零实现。codex-rs 也使用了类似的文本编辑组件模式。

**替代方案**: 自己实现——工作量大且容易出 bug（光标定位、Unicode 处理等）。

### D5: 应用状态机设计

**选择**: `App` 结构体持有所有 TUI 状态（消息历史、输入缓冲、滚动偏移），`render.rs` 只负责根据 App 状态绘制 UI。

**理由**: 状态与渲染分离是 ratatui 的推荐模式（Elm architecture）。App 处理事件并更新状态，render 函数是纯渲染。

### D6: Keyboard Enhancement Flags

**选择**: 启用 crossterm 的 keyboard enhancement flags 以区分 Enter 和 Shift+Enter。

**理由**: 参考 codex-rs `tui.rs` 第 66-79 行，需要 `DISAMBIGUATE_ESCAPE_CODES` 和 `REPORT_EVENT_TYPES` 来识别修饰键组合。不支持的终端会优雅降级。

## Risks / Trade-offs

- **[tui-textarea 依赖]** → 引入额外依赖，但该 crate 成熟稳定，减少自行实现的 bug 风险
- **[Keyboard enhancement 不兼容]** → 部分旧终端不支持 Shift+Enter 区分。→ 优雅降级：不支持时 Enter 直接发送，无法多行输入（可接受的 MVP 行为）
- **[Windows 终端兼容性]** → Windows 上 crossterm 行为可能有差异。→ 使用 `static_vcruntime` 静态链接，开发时在 Windows Terminal 测试
- **[.krew/logs/ 权限]** → 自动创建目录可能因权限失败。→ 启动时提前检查并给出清晰错误信息
