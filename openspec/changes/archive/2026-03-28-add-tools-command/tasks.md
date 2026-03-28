## 1. SlashCommand enum 扩展

- [x] 1.1 在 `crates/krew-core/src/command.rs` 的 `SlashCommand` enum 中新增 `Tools` variant
- [x] 1.2 在 `from_input()` 中添加 `"/tools"` 匹配
- [x] 1.3 在 `name()`、`description()`、`all_help()` 中添加 `/tools` 条目（确保 tab 补全和 /help 自动覆盖）

## 2. execute_tools handler 实现

- [x] 2.1 在 `crates/krew-cli/src/app/commands.rs` 的 `execute_slash_command()` match 中添加 `SlashCommand::Tools` arm
- [x] 2.2 实现 `execute_tools()` 方法：遍历 `config.agents`，从 `self.agents` 获取对应 `AgentRuntime`，用 `is_mcp_tool()` 过滤 MCP 工具，按设计格式渲染输出
- [x] 2.3 区分三种 agent 状态：有工具（显示 `N tool(s)` + 工具列表）、tools=false（显示 `no tool(s)`）、初始化失败即不在 `self.agents` 中（显示 `unavailable`）

## 3. 验证

- [x] 3.1 运行 `cargo fmt --all` 和 `cargo clippy` 确保代码质量
- [x] 3.2 运行 `cargo test` 确保无回归
