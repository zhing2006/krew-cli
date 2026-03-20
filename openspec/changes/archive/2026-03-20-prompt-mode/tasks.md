## 1. CLI 参数与入口改造

- [x] 1.1 在 `Cli` struct 中新增 `-p / --prompt` 参数（`Option<String>`）和 `--format` 参数（`Option<String>`，默认 "text"）
- [x] 1.2 将 `main()` 改为 `fn main()` + `fn run() -> i32` 模式，通过 `std::process::exit(code)` 支持精确 exit code（0/1/2）
- [x] 1.3 在 `run()` 中添加参数校验：`-p` 与 `--resume` 互斥检查；`--format` 值校验（仅 "text" / "json"）；校验失败输出 stderr 并返回 2
- [x] 1.4 在 `run()` 中根据 `cli.prompt` 分支：有值进入 prompt 模式，无值进入 TUI 模式（TUI 分支错误统一返回 1）

## 2. prompt_mode 模块核心

- [x] 2.1 创建 `crates/krew-cli/src/prompt_mode.rs` 模块，定义 `OutputFormat` 枚举（Text/Json）和 `run_prompt_mode()` 异步入口函数，返回 `i32`（exit code）
- [x] 2.2 实现寻址解析（仅对 `-p` 原始参数调用 `parse_input()`，不含 stdin 内容）；`LastRespondent` 时返回 exit code 2（覆盖纯未知 @token 场景如 `@nonexistent`）
- [x] 2.3 实现 stdin 管道检测（`std::io::stdin().is_terminal()`）和读取逻辑，将 stdin 内容以 `<stdin>...</stdin>` 包裹，与原始 prompt（含 @token）拼接为最终消息 body
- [x] 2.4 实现 agent 初始化：调用 `init_agents()`，覆盖所有 agent 的 `approval_mode` 为 `FullAuto`，构建 dispatch queue（`resolve_dispatch_queue()`）

## 3. AgentEvent 消费与输出

- [x] 3.1 实现 AgentEvent 消费循环：接收 `start_completion()` 返回的 rx，逐事件处理全部变体
- [x] 3.2 实现 Text 格式 streaming 输出：`ResponseStart` → `[agent_name]` header；`TextDelta` → 立即 `print!()` + `flush()`；`ThinkingDelta` → 静默丢弃；`ServerToolStart/Done` → `🌐` 格式；`ToolCallStart` → `⚡ tool(args)`；`ToolCallOutput` → 4 空格缩进；`ToolCallDone` → `⎿  summary`；`Done` → 换行收尾
- [x] 3.3 实现 JSON 格式非 streaming 输出：`TextDelta` → 缓存到 buffer；`Done` → 输出 `{"type":"text","content":"..."}` JSONL；工具/server tool 事件即时输出对应 JSONL 行
- [x] 3.4 实现 ApprovalRequest 自动回复：收到 ApprovalRequest 时直接发送 `Approved`
- [x] 3.5 实现 Error 事件处理：将错误输出到 stderr，标记 has_error flag
- [x] 3.6 实现 Retrying 事件处理：输出重试信息到 stderr

## 4. 多 Agent 调度与 AI-to-AI

- [x] 4.1 实现多 agent 顺序调度：按 dispatch queue 逐个启动 agent，某个 agent 出错后继续下一个（与 TUI 模式一致）
- [x] 4.2 实现 AI-to-AI routing：agent 响应中检测 @mention，按配置策略（immediate/queued）更新 dispatch queue，遵循 `agent_to_agent_max_rounds` 轮次上限
- [x] 4.3 实现最终 exit code 计算：全部成功返回 0，任一 agent 出错返回 1

## 5. Session 持久化与 MCP

- [x] 5.1 实现 session 保存：复用 `krew_core::persistence` 将完整对话（含中间消息、token 用量）保存到 `.krew/sessions/`
- [x] 5.2 实现 MCP 初始化：配置了 MCP servers 时启动连接、注册工具到 agent registries，执行完成后通过 Drop 清理

## 6. 集成

- [x] 6.1 在 `main.rs` 中集成完整的 prompt 模式分支，包括 config 加载、logging 初始化、tokio runtime 构建

## 7. 单元测试

- [x] 7.1 测试 stdin 内容拼接：验证 `<stdin>...</stdin>` 包裹逻辑，空 stdin 不拼接，有内容时正确拼接到 prompt 前方（寻址不受 stdin 内容影响）
- [x] 7.2 测试寻址校验：无已知 `@agent` 前缀（含 `@nonexistent`）返回 LastRespondent → 错误；`@agent` / `@all` / `@agent1 @agent2` 正常通过；`@claude explain @dataclass` 正常路由到 claude
- [x] 7.4 测试 Text 格式化输出：验证 `[agent_name]` header、工具调用行、server tool 行、shell output 缩进、ThinkingDelta 静默丢弃
- [x] 7.5 测试 JSON 格式化输出：验证 JSONL 各事件类型（tool_start、tool_output、tool_done、server_tool_start、server_tool_done、text）的 JSON 结构和字段完整性
- [x] 7.6 测试 OutputFormat 解析：`"text"` → Text，`"json"` → Json，其他值报错
- [x] 7.7 测试 AgentEvent 消费：模拟发送完整事件序列（包含 ThinkingDelta、ServerToolStart/Done、ToolCallOutput、Retrying），验证 Text/JSON 两种格式的输出内容
- [x] 7.8 测试多 agent 错误处理：模拟 agent A 出错 + agent B 成功，验证 B 仍正常输出且 exit code 为 1

## 8. 集成测试

- [x] 8.1 编写 CLI 集成测试：验证 `-p` 与 `--resume` 互斥、`--format` 非法值、缺少 @ 寻址等参数校验场景的 exit code
