## Why

用户在多轮对话中经常需要回退到某个节点重新提问，例如当 AI 回答偏离方向或用户想尝试不同的提问方式。目前没有办法在不丢失整个会话的情况下做到这一点——只能 `/clear` 重新开始或手动删除 session 文件。`/rewind` 提供了一种优雅的"对话分支"能力，让用户可以从所有用户消息中选择一个作为回退点。

## What Changes

- 新增 `/rewind` slash 命令，弹出 popup 列表展示所有用户消息
- 用户选择某条消息后，截断对话历史到该消息之前（删除该消息及其后所有内容）
- 清屏并重放截断后的对话历史
- Rewind 后不立即保存 session，原始 session 文件保持不变
- 用户发送新消息时自动生成新 session ID，形成"对话分支"
- 如果选择了第一条用户消息（等于清空所有对话），复用 `/clear` 逻辑

## Capabilities

### New Capabilities
- `rewind-command`: `/rewind` 命令的完整行为——popup 选择、对话截断、屏幕重放、以及 fork 语义（新消息产生新 session ID）

### Modified Capabilities
- `slash-commands`: 新增 `/rewind` 到内置命令列表和 `/help` 输出
- `session-lifecycle`: 新增 rewound 状态标记，控制 rewind 后首次发消息时生成新 session ID

## Impact

- `krew-core/src/command.rs` — SlashCommand enum 新增 Rewind variant
- `krew-cli/src/app/state.rs` — App struct 新增 `rewound: bool` 字段
- `krew-cli/src/app/commands.rs` — 新增 `execute_rewind()` 实现 + 提取可复用的消息重放方法
- `krew-cli/src/app/message.rs` — `send_message()` / `send_expanded_text()` 入口检查 rewound 标记
- `krew-cli/src/completion.rs` — ActivePopup 新增 RewindPicker variant
- `krew-cli/src/app/input.rs` — 处理 RewindPicker 的选择确认
