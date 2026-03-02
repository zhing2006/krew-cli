## Why

krew-cli 目前只支持单 Agent 回复（@all 和 @multiple 都退化为选第一个 Agent）。Phase 6 需要实现多 Agent 协作的核心能力：@all 广播串行执行、@multiple 按指定顺序执行、上下文共享（后续 Agent 可见前面 Agent 的回复）、错误隔离，以及无前缀输入延续上一个回答者。

## What Changes

- 实现 `@all` 广播：按 `reply_order` 顺序串行执行每个 Agent 的完整回合
- 实现 `@name1 @name2` 多 Agent 寻址：按 `@` 出现顺序串行执行
- 实现无前缀延续（`LastRespondent`）：发给上一个回复的 Agent，无回答者时提示用户指定
- 上下文共享：前一个 Agent 的回复追加到消息历史，后续 Agent 可见
- 错误隔离：@all 场景下某个 Agent 失败不影响其他 Agent 继续
- Token 用量累计：每个 Agent 完成后累加到会话统计
- **BREAKING** 删除 `use_name_field` 配置：统一使用 `[agent_name]` 前缀标识其他 Agent 的回复（因为连续 other-agent 消息会被合并为一条 user 消息，name 字段无法归属）
- 所有 provider 的 `convert_messages` 统一支持 `OtherAgentRole` 参数（目前仅 OpenAI Chat 支持），`OtherAgentRole` 在全局 `Settings` 中配置
- OpenAI Chat 的 `convert_messages` 补上 `merge_consecutive_same_role` 调用

## Capabilities

### New Capabilities
- `multi-agent-dispatch`: App 层多 Agent 串行调度（pending_agents 队列、链式触发、错误隔离、LastRespondent 追踪）

### Modified Capabilities
- `llm-common`: 删除 `use_name_field` 相关逻辑；所有 provider 的 `convert_messages` 统一接受 `OtherAgentRole` 参数
- `openai-chat-client`: 删除 `use_name_field` 参数；添加 `merge_consecutive_same_role` 调用
- `anthropic-client`: `convert_messages` 添加 `OtherAgentRole` 参数
- `google-client`: `convert_messages` 添加 `OtherAgentRole` 参数
- `openai-responses-client`: `convert_messages` 添加 `OtherAgentRole` 参数
- `config-types`: 从 `ProviderConfig` 中删除 `use_name_field` 字段；在 `Settings` 中新增 `other_agent_role` 字段
- `agent-loop`: 从 `AgentRuntime` 中删除 `use_name_field` 字段及相关 system prompt hint

## Impact

- **krew-cli/src/app/state.rs**: 新增 `pending_agents`、`last_respondent` 字段；`handle_agent_event(Done/Error)` 中链式触发下一个 Agent
- **krew-cli/src/app/message.rs**: `send_message()` 路由逻辑改为填充队列并启动首个 Agent
- **krew-core/src/agent.rs**: 删除 `use_name_field` 字段和相关 system prompt hint
- **krew-llm/src/openai_chat.rs**: 删除 `use_name_field`，添加 merge，统一 `[agent_name]` 前缀
- **krew-llm/src/anthropic.rs**: `convert_messages` 签名变更，添加 `OtherAgentRole` 参数
- **krew-llm/src/google.rs**: 同上
- **krew-llm/src/openai_responses.rs**: 同上
- **krew-config/src/lib.rs**: 删除 `ProviderConfig.use_name_field`
- **BREAKING**: 已有 `settings.toml` 中的 `use_name_field` 字段将被忽略（TOML 反序列化默认忽略未知字段，无需迁移）
