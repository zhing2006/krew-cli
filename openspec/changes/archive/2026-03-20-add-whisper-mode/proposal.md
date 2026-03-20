## Why

目前 krew-cli 会话中的所有消息对每个 agent 都可见。用户无法在不让其他 agent 看到内容的情况下与特定 agent 进行私密对话。这限制了策略性使用场景——例如让一个 agent 评价另一个 agent 的工作而不影响讨论，或仅与一个 agent 分享敏感上下文。

## What Changes

- **新增 `#agent` 语法**：`#name message` 向指定 agent 发送密语（私密消息）。其他 agent 仅看到 `[whisper to name]` 占位符。
- **密语组**：`#a #b message` 创建私密组——成员互相可见消息，非成员看到占位符。
- **密语回复继承**：agent 对密语的回复自动标记为相同组的密语，形成密语对。
- **组内 A2A 路由**：密语组内的 agent 可以互相 `@mention`。组外 agent 的 mention 被静默忽略。
- **`#all` 被禁止**：对所有 agent 密语在语义上等同于普通消息；解析器以错误拒绝。
- **`LastRespondent` 不继承密语**：不带 `#` 前缀的后续消息回到普通（非密语）模式，即使上一个回复者在密语对话中。
- **密语消息不参与压缩**：密语对从 `/compact` 压缩区提取并保留，维持会话中的隐私边界。
- **TUI 视觉区分**：密语消息在用户提示行和 agent 响应 header 中显示锁图标。
- **Prompt 模式 (`-p`) 支持**：`#agent` 语法在非交互模式下行为一致。

## Capabilities

### New Capabilities
- `whisper-parsing`：从用户输入解析 `#agent` 语法，与现有 `@agent` 解析逻辑共享
- `whisper-visibility`：在 `prepare_messages_for_agent` 中过滤密语消息，使组外 agent 仅看到占位符
- `whisper-display`：TUI 渲染密语指示符（用户消息和 agent header 上的锁图标）

### Modified Capabilities
- `input-routing`：`parse_input` 扩展以识别 `#` 前缀，返回 `Addressee` 同时附带 whisper 标志
- `message-types`：`ChatMessage` 新增 `whisper_targets: Option<Vec<String>>` 字段
- `session-storage`：`MessageEntry` 新增 `whisper_targets: Option<Vec<String>>`（TOML 原生数组）用于持久化
- `agent-to-agent-routing`：密语模式下 A2A mention 过滤限制为密语组成员
- `multi-agent-dispatch`：调度队列将 `whisper_targets` 传播到 agent 响应
- `compact`：压缩逻辑从压缩区提取带 `whisper_targets` 的消息并保留
- `prompt-mode`：`run_prompt_mode` 以与 TUI 相同的密语语义处理 `#agent`

## Impact

- **krew-core**：`router.rs`（parse_input、Addressee）、`agent/prepare.rs`（过滤）、`agent/mod.rs`（system prompt、密语传播）、`agent/agent_loop.rs`（消息标记）、`compact.rs`（提取逻辑）
- **krew-llm**：`ChatMessage` 结构体（新字段）
- **krew-storage**：`MessageEntry` 结构体（新字段）、序列化
- **krew-cli**：`app/message.rs`（TUI 发送/显示）、`prompt_mode/mod.rs`（P 模式支持）、渲染
- **krew-config**：`validate()` 中新增 agent 名称校验——禁止 `"all"` 作为 agent 名称（`@all` 和 `#all` 均为保留字）
- **无新依赖**
- **无破坏性变更**——现有 `@` 语法和行为不变
