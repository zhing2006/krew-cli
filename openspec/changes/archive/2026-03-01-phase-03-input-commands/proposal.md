## Why

Phase 1-2 已完成 TUI 框架和配置系统，但用户输入目前只有 echo 回显，无法区分消息发送目标，也无法执行任何命令。Phase 3 需要让 `@` 寻址和 `/` 命令真正工作起来，这是后续 LLM 接入（Phase 4）的前置条件。

## What Changes

- 将 `parse_input()` 集成到 TUI 输入流，解析 `@all`/`@name`/无前缀 三种寻址模式
- 添加 Agent 名称校验函数，`@unknown` 时报错提示
- 实现 Slash 命令执行：`/help` `/agents` `/clear` `/quit` 正常工作，`/new` `/resume` `/compact` 显示"功能待实现"占位提示
- `/agents` 输出 Agent 列表及 token 统计（占位显示 0）
- 补全弹窗系统：输入 `/` 触发命令补全、输入 `@` 触发 Agent 名称补全，弹窗替换状态栏区域，viewport 动态扩展（与 codex 行为一致）
- Echo 升级：回显时显示解析结果标记 `[→ @all]` / `[→ @gpt]` / `[→ last]`

## Capabilities

### New Capabilities
- `input-routing`: @ 寻址解析集成、Agent 名称校验、解析结果标记显示
- `slash-commands`: Slash 命令识别与执行（/help /agents /clear /quit + 占位命令）
- `completion-popup`: 补全弹窗 UI 组件，/ 命令补全和 @ Agent 补全，viewport 动态扩展

### Modified Capabilities
- `tui-framework`: 输入处理流程重构——从硬编码 echo 改为解析分发，viewport 布局支持弹窗区域

## Impact

- **krew-core**: `router.rs` 添加校验函数，`command.rs` 不变（已有完整枚举和解析）
- **krew-cli**: `app.rs` 重构 `send_message()` 和 `handle_key()`，`render.rs` 添加命令输出和弹窗渲染，新增 `completion.rs` 模块
- **依赖**: 无新增外部依赖
