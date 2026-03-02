## Context

krew-cli 目前只支持单 Agent 回复。`send_message()` 中 `@all` 和 `@multiple` 都退化为选择 `reply_order` 中第一个可用 Agent。Phase 5 已完成所有 LLM Provider 接入，消息格式转换（`convert_messages`）已支持 other-agent 识别和 `[agent_name]` 前缀，三个 provider 已有 `merge_consecutive_same_role` 合并。

现有架构中 App 是事件驱动的：`agent_event_rx` 通道接收 `AgentEvent`，在 `handle_agent_event()` 中处理。流式渲染管线（`MarkdownStreamCollector` → `StreamState` → `AdaptiveChunkingPolicy`）已完善。

## Goals / Non-Goals

**Goals:**
- 实现 @all 按 `reply_order` 串行执行多个 Agent
- 实现 @multiple 按用户 `@` 出现顺序串行执行
- 实现 LastRespondent 追踪（无前缀输入发给上一个回答者）
- 错误隔离：某个 Agent 失败不阻塞后续 Agent
- Token 用量正确累计到每个 Agent
- 删除 `use_name_field`，统一 `[agent_name]` 前缀方式
- 所有 provider 的 `convert_messages` 统一支持 `OtherAgentRole` 参数

**Non-Goals:**
- 工具调用（Phase 8/9）
- 会话持久化（Phase 7）
- Agent 间 @对方（v0.4 方向）
- 并行执行多个 Agent（设计上始终串行）

## Decisions

### D1: 方案 A — App 层事件驱动链式触发

**选择**: 在 App 上新增 `pending_agents: VecDeque<String>` 队列。`send_message()` 填充队列并启动首个 Agent，`handle_agent_event(Done)` 中弹出下一个并启动。

**替代方案**: krew-core 层封装 `MultiAgentLoop`。但工具审批需要 TUI 交互（Phase 8/9），若编排逻辑在 core 层则需要回调通道，增加复杂度。App 层链式触发改动最小、渐进性好，且与未来工具调用的审批流天然适配。

**排序规则**:
- `@all` → `reply_order` 中所有有 LLM client 的 Agent，按 `reply_order` 顺序
- `@a @b @c` → 按用户输入中 `@` 出现的顺序，不重排为 `reply_order`
- `@name` → 单个 Agent
- 无前缀 → `last_respondent`

### D2: LastRespondent 追踪

App 新增 `last_respondent: Option<String>` 字段。

- 更新时机：`handle_agent_event(Done)` 保存当前 agent_name
- 无前缀输入时：有 `last_respondent` 则发给它，没有则显示错误提示用户使用 `@name` 指定

### D3: 删除 use_name_field，统一 [agent_name] 前缀

**原因**: 连续 other-agent 消息在 `merge_consecutive_same_role` 后合并为单条 user 消息，`name` 字段无法归属给多个 agent。只有内容中的 `[agent_name]` 前缀能在合并后保留身份信息。

**影响**: 删除 `ProviderConfig.use_name_field`（krew-config）、`AgentRuntime.use_name_field`（krew-core）、`OpenAiChatClient.use_name_field` 及 `convert_messages` 的 `use_name_field` 参数（krew-llm）、`init_agents()` 中的传递代码（krew-cli）。

system prompt 中的 hint 统一为："Their messages are prefixed with [agent_name] in the content."

### D4: 所有 provider 统一支持 OtherAgentRole

当前只有 OpenAI Chat 的 `convert_messages` 接受 `OtherAgentRole` 参数，其他三个 provider 硬编码 `"user"`。

**变更**: Anthropic、Google、OpenAI Responses 的 `convert_messages` 签名统一增加 `other_agent_role: &OtherAgentRole` 参数。默认值为 `OtherAgentRole::User`（行为不变），但允许未来配置为 `OtherAgentRole::Assistant`。

`OtherAgentRole` 在全局 `Settings` 中配置（新增 `other_agent_role` 字段，默认 `User`），由各 LLM client struct 持有。`init_agents()` 从 `config.settings.other_agent_role` 获取并传递给每个 client。

### D5: OpenAI Chat 补上 merge_consecutive_same_role

OpenAI Chat 的 `convert_messages` 是唯一没有调用 `merge_consecutive_same_role` 的 provider。虽然 OpenAI API 不强制要求 role 交替，但为了一致性和多 Agent 场景的正确性，需要补上。

### D6: 错误隔离

`handle_agent_event(Error)` 中：
- 显示错误信息（当前已有）
- 检查 `pending_agents` 队列，如果还有下一个 Agent 则继续启动
- 如果队列为空则正常结束，解锁输入

### D7: 输入锁定

多 Agent 执行期间，`agent_event_rx` 始终有值（一个 Agent 完成后立刻设置为下一个的 rx），因此用户无法发送新消息。在 `AllDone`（队列清空且最后一个 Agent Done）后 `agent_event_rx` 设为 None，解锁输入。

## Risks / Trade-offs

- **[Risk] OtherAgentRole 配置项新增** → 新增 `Settings.other_agent_role` 字段（全局配置）。TOML 反序列化默认忽略未知字段，向后兼容。
- **[Risk] use_name_field 删除是 BREAKING** → 现有 settings.toml 中若有此字段会被忽略（`serde(default)` + `deny_unknown_fields` 未启用）。影响极小。
- **[Risk] 多 Agent 场景下长时间占用终端** → @all 串行执行可能耗时较长（N 个 Agent × 每个的响应时间）。用户需等待全部完成。Phase 12 计划加入 Ctrl+C 中断，届时可中断当前 Agent 并跳过后续。
