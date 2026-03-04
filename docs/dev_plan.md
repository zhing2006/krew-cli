# krew-cli — 开发计划

> 版本: 0.1.0 | 日期: 2026-02-28
> 参考: [PDD](./PDD.md) | [TDD](./TDD.md)

---

## 原则

- **每个 Phase 产出一个可运行的二进制**，逐步叠加能力
- 每个 Phase 范围尽量小，独立可验收
- Phase 之间顺序依赖，后面的 Phase 建立在前面的基础上

---

## Phase 总览

| Phase | 名称 | 详情 | 状态 | 已有基础 |
| ----- | ---- | ---- | ---- | -------- |
| 1 | [日志系统 + TUI Echo 模式](./phases/phase-01-tui-echo.md) | tracing 日志、ratatui TUI、多行输入、Echo 回显 | ✅ 已完成 | clap 参数解析、App 结构体骨架 |
| 2 | [配置系统](./phases/phase-02-config.md) | 加载 settings.toml、CLI 参数覆盖、AGENTS.md | ✅ 已完成 | Config/AgentConfig 等类型定义、AGENTS.md 加载已完成（含测试） |
| 3 | [输入解析 + Slash 命令](./phases/phase-03-input-commands.md) | @ 寻址、/help /agents /clear 等命令 | ✅ 已完成 | parse_input() + Addressee 枚举、SlashCommand 枚举骨架已有 |
| 4 | [单 LLM 接入 + Markdown 渲染](./phases/phase-04-openai-chat.md) | OpenAI Chat Completions、流式输出、syntect 高亮 | ✅ 已完成 | LlmClient trait + StreamEvent + Usage 已定义 |
| 5 | [更多 LLM Provider](./phases/phase-05-more-providers.md) | Anthropic、Google、OpenAI Responses、Compatible、Azure | ✅ 已完成 | 各 Provider 文件已创建（仅 header stub） |
| 6 | [多 Agent 协作](./phases/phase-06-multi-agent.md) | @all 串行执行、上下文共享、错误隔离 | ✅ 已完成 | AgentRuntime 结构体、build_system_prompt() 已实现（含测试） |
| 7 | [会话持久化](./phases/phase-07-session-persistence.md) | TOML 存储、/new、/resume、实时保存 | ✅ 已完成 | session_file.rs + history_file.rs 实现、/new /resume 命令、--resume CLI、输入历史持久化 |
| 8 | [工具系统 — 只读](./phases/phase-08-tools-readonly.md) | read_file、glob、grep、Agent Loop 工具调用 | ✅ 已完成 | ToolSpec/ToolHandler/ToolRegistry、3 个只读工具、Agent Loop 工具循环、4 Provider 工具消息转换、TUI 渲染 |
| 9 | [工具系统 — 写入 + 审批](./phases/phase-09-tools-write-approval.md) | write_file、edit_file、shell、审批流 | ✅ 已完成 | write_file/edit_file/shell 工具、审批 overlay、会话级审批缓存、命令级 shell 审批、可配置免审批列表、流式 shell 输出、diff 预览 |
| 10 | [MCP 集成](./phases/phase-10-mcp.md) | MCP Client、工具发现、信任级别 | ✅ 已完成 | rmcp SDK 集成、McpClient/McpManager/McpToolHandler、annotations 审批、TUI 显示名转换 |
| 11 | [Compact + Token 管理](./phases/phase-11-compact-tokens.md) | /compact、自动压缩、/agents token 统计 | ⬜ 待开始 | — |
| 12 | [交互打磨](./phases/phase-12-interaction-polish.md) | @ / 补全、思考过程、Ctrl+C 中断、Web Search、Fetch | ⬜ 待开始 | — |
| 13 | [静态链接 + 发布](./phases/phase-13-release.md) | 三平台静态链接、CI/CD、二进制优化 | ⬜ 待开始 | — |
