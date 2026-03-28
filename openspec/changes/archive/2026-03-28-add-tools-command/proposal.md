## Why

目前 `/agents` 显示 agent 列表，`/mcp` 显示 MCP 工具，`/skills` 显示 skill，但没有一个命令能回答"某个 agent 到底能用哪些非 MCP 的 runtime tools"。用户需要一个 `/tools` 命令来查看每个 agent 的工具配置全貌。

## What Changes

- 新增 `/tools` slash command，按 agent 分组列出每个 agent 可用的非 MCP runtime tools（built-in、sub-agent 等）
- `tools=false` 的 agent 显示 `no tool(s)`；初始化失败的 agent 显示 `unavailable`
- MCP 工具不在此命令中显示（已有 `/mcp`）
- 每个工具显示名称和描述
- `/tools` 出现在 `/help` 列表和 tab 补全中

## Capabilities

### New Capabilities
- `tools-command`: `/tools` slash command 的定义、解析和 TUI 渲染

### Modified Capabilities
- `slash-commands`: 在 `SlashCommand` enum 中新增 `Tools` variant 及相关解析/帮助信息

## Impact

- `crates/krew-core/src/command.rs` — 新增 `SlashCommand::Tools` variant
- `crates/krew-cli/src/app/commands.rs` — 新增 `execute_tools()` handler
- 无新依赖、无 API 变更、无 breaking change
