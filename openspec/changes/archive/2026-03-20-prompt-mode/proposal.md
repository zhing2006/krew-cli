## Why

krew-cli 目前只支持交互式 TUI 模式，无法在脚本、CI/CD 管道或与其他 CLI 工具组合使用。实现类似 Claude Code 的 `-p` (prompt) 模式，让用户可以通过命令行参数传入 prompt、接收纯文本输出、进程自动退出，从而支持非交互式自动化场景。

## What Changes

- 新增 `-p <prompt>` CLI 参数，进入非交互式 prompt 模式
- 支持 stdin 管道输入（`cat file | krew -p "@agent review"`），内容以 `<stdin>...</stdin>` 包裹注入 prompt
- 必须包含 `@agent` 或 `@all` 寻址，否则报错退出（TUI 模式允许 LastRespondent，`-p` 模式不允许，因为无之前的对话上下文）
- Text 格式默认 streaming 输出（逐 delta 实时打印），JSON 格式非 streaming
- 多 agent 按 `reply_order` 顺序输出，每个 agent 输出前显示 `[name]` header
- 新增 `--format` 参数：默认 `text`（人类可读），可选 `json`（JSONL 结构化输出）
- 工具调用强制 `FullAuto` 模式，无需用户审批
- 支持 AI-to-AI routing（agent 回复中 @mention 其他 agent）
- 多 agent 出错时继续执行剩余 agent（与 TUI 模式一致），最终 exit code 反映是否有错误
- Session 持久化行为与 TUI 模式一致
- Exit code: 0 成功 / 1 agent 错误 / 2 解析/配置错误

## Capabilities

### New Capabilities
- `prompt-mode`: 非交互式 prompt 执行模式，包括 CLI 参数解析、stdin 管道读取、AgentEvent 消费（streaming text / buffered json）、session 保存

### Modified Capabilities

_(无现有 capability 的需求变更)_

## Impact

- `krew-cli` crate: 新增 `-p` 和 `--format` CLI 参数，新增 `prompt_mode.rs` 模块，修改 `main.rs` 入口（`fn main()` → `fn run() -> i32` + `process::exit` 支持精确 exit code）
- `krew-core` crate: 零改动（AgentRuntime/AgentEvent 接口已足够解耦）
- 依赖: 无新增（使用 `std::io::IsTerminal` trait 检测 stdin，Rust 1.70+ 标准库自带）
