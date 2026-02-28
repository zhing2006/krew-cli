## Why

项目已有 Cargo workspace 骨架、clap 参数解析和 App 结构体，但运行后无任何用户可见的界面。Phase 1 需要搭建 TUI 框架和日志基础设施，让用户第一次看到可交互的终端界面——输入文本后 echo 回显，为后续接入 LLM 打下基础。

## What Changes

- 初始化 `tracing` + `tracing-subscriber` 日志系统，日志写入 `.krew/logs/` 文件（不输出到终端）
- 基于 `ratatui` + `crossterm` 搭建全屏 TUI 界面：上方可滚动输出区域 + 下方 `you>` 输入框
- 启动时显示 ASCII banner（PDD §5.1 定义的 logo）
- 实现多行输入：Shift+Enter 换行，Enter 发送
- Echo 模式：用户输入文本后原样回显到输出区域（临时模式，后续替换为 LLM 调用）
- 支持 `/quit` 命令和 `Ctrl+C` 退出程序
- `--verbose` 参数控制日志级别（debug vs info）

## Capabilities

### New Capabilities
- `logging`: tracing 日志初始化，文件日志输出到 `.krew/logs/`，日志级别受 `--verbose` 控制
- `tui-framework`: ratatui 全屏 TUI 框架，包含输出区域、输入框、banner 显示、事件循环
- `echo-mode`: 临时 echo 回显模式，用户输入原样输出到聊天区域

### Modified Capabilities
- `cli-args`: 将已有的 clap 参数解析与新的日志系统和 TUI 启动流程串联

## Impact

- **代码变更**: `crates/krew-cli/` 下的 `main.rs`、`app.rs`、`render.rs` 将被大幅重写
- **新依赖**: `crossterm`（通过 ratatui 的 crossterm feature 引入，已在 Cargo.toml 中配置）、`tracing-appender`（用于文件日志输出）
- **文件系统**: 运行时会自动创建 `.krew/logs/` 目录
- **现有功能**: App::new() 中的 AGENTS.md 加载逻辑需要保留并集成到新的启动流程中
