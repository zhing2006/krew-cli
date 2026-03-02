## MODIFIED Requirements

### Requirement: OpenAI Responses 消息格式转换
客户端 SHALL 将统一 `ChatMessage` 转换为 Responses API 的 `input` 数组格式。`convert_messages` SHALL 接受 `other_agent_role: &OtherAgentRole` 参数，根据该参数决定 other-agent 消息的 role。

#### Scenario: user 消息转换
- **WHEN** `ChatRole::User`
- **THEN** 转换为 `{"type": "message", "role": "user", "content": "..."}`

#### Scenario: system 消息转换
- **WHEN** `ChatRole::System`
- **THEN** 转换为 `{"type": "message", "role": "developer", "content": "..."}`

#### Scenario: 当前 Agent 回复转换
- **WHEN** `ChatRole::Assistant` 且为当前 agent
- **THEN** 转换为 `{"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "..."}], "status": "completed"}`

#### Scenario: 其他 Agent 回复使用 OtherAgentRole
- **WHEN** `ChatRole::Assistant` 且 agent_name != 当前 agent，`other_agent_role` 为 `User`
- **THEN** 转换为 `{"type": "message", "role": "user", "content": "[agent_name] ..."}`

#### Scenario: OtherAgentRole 为 Assistant
- **WHEN** `ChatRole::Assistant` 且 agent_name != 当前 agent，`other_agent_role` 为 `Assistant`
- **THEN** 转换为 `{"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "[agent_name] ..."}], "status": "completed"}`

#### Scenario: 连续同 role 消息合并
- **WHEN** 转换后存在连续相同 role 的消息
- **THEN** 使用 `merge_consecutive_same_role` 合并，content 用 `\n\n` 连接
