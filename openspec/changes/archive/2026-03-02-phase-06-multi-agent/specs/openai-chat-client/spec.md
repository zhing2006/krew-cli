## MODIFIED Requirements

### Requirement: 消息格式转换
客户端 SHALL 将统一的 `ChatMessage` 列表转换为 OpenAI Chat Completions 的 `messages` 数组格式。转换后 SHALL 调用 `merge_consecutive_same_role` 合并连续相同 role 的消息。

#### Scenario: 基本角色映射
- **WHEN** `ChatMessage` 包含 role System/User/Assistant/Tool
- **THEN** 映射为 OpenAI `"system"`/`"user"`/`"assistant"`/`"tool"` role

#### Scenario: 其他 Agent 回复的 role 处理
- **WHEN** `ChatMessage` role 为 Assistant 且 agent_name != 当前 agent
- **THEN** 根据 `other_agent_role` 参数决定使用 `"user"` 或 `"assistant"` role，content 前缀添加 `[agent_name]`

#### Scenario: 连续同 role 消息合并
- **WHEN** 转换后存在连续相同 role 的消息（例如两条连续 user 消息）
- **THEN** 合并为单条消息，content 用 `\n\n` 连接

## REMOVED Requirements

### Requirement: use_name_field 支持
**Reason**: 连续 other-agent 消息合并后 `name` 字段无法归属，统一使用 `[agent_name]` content 前缀。
**Migration**: `convert_messages` 删除 `use_name_field` 参数，`OpenAiChatClient` struct 删除 `use_name_field` 字段。所有 other-agent 消息统一使用 `[agent_name]` content 前缀。
