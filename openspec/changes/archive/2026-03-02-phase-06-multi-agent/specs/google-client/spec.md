## MODIFIED Requirements

### Requirement: Gemini 消息格式转换
客户端 SHALL 将统一 `ChatMessage` 转换为 Gemini `contents` 格式（role 使用 `user`/`model`），system 消息分离到 `systemInstruction` 字段。`convert_messages` SHALL 接受 `other_agent_role: &OtherAgentRole` 参数，根据该参数决定 other-agent 消息的 role。

#### Scenario: role 映射
- **WHEN** `ChatRole::Assistant` 为当前 agent
- **THEN** 映射为 `"role": "model"`

#### Scenario: System 消息分离
- **WHEN** Messages 包含 `ChatRole::System`
- **THEN** 分离为请求体 `systemInstruction: {"parts": [{"text": "..."}]}`

#### Scenario: 其他 Agent 回复使用 OtherAgentRole
- **WHEN** Message role 为 Assistant 且 agent_name != 当前 agent，`other_agent_role` 为 `User`
- **THEN** 转换为 `"role": "user"` 并添加 `[agent_name]` content 前缀

#### Scenario: OtherAgentRole 为 Assistant
- **WHEN** Message role 为 Assistant 且 agent_name != 当前 agent，`other_agent_role` 为 `Assistant`
- **THEN** 转换为 `"role": "model"` 并添加 `[agent_name]` content 前缀

#### Scenario: 连续同 role 消息合并
- **WHEN** 转换后存在连续相同 role 的消息
- **THEN** 使用 `merge_consecutive_same_role` 合并，parts text 用 `\n\n` 连接
