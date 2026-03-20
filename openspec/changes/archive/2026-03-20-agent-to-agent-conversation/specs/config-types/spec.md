## MODIFIED Requirements

### Requirement: Settings 结构体
`krew-config` SHALL 定义 `Settings` 结构体，包含字段：`approval_mode: ApprovalMode`、`reply_order: Vec<String>`、`auto_compact_threshold: Option<u32>`、`other_agent_role: OtherAgentRole`（默认 `User`）、`agent_to_agent_routing: AgentToAgentRouting`（默认 `Immediate`）、`agent_to_agent_max_rounds: u32`（默认 `10`）。

#### Scenario: Settings 字段齐全
- **WHEN** 构造 `Settings` 值
- **THEN** 所有字段 SHALL 存在且类型正确

#### Scenario: other_agent_role 默认值
- **WHEN** settings TOML 块未包含 `other_agent_role` 字段
- **THEN** 默认值 SHALL 为 `OtherAgentRole::User`

#### Scenario: other_agent_role 配置为 assistant
- **WHEN** settings TOML 块包含 `other_agent_role = "assistant"`
- **THEN** SHALL 反序列化为 `OtherAgentRole::Assistant`

#### Scenario: agent_to_agent_routing 默认值
- **WHEN** settings TOML 块未包含 `agent_to_agent_routing` 字段
- **THEN** 默认值 SHALL 为 `AgentToAgentRouting::Immediate`

#### Scenario: agent_to_agent_max_rounds 默认值
- **WHEN** settings TOML 块未包含 `agent_to_agent_max_rounds` 字段
- **THEN** 默认值 SHALL 为 `10`

## ADDED Requirements

### Requirement: AgentToAgentRouting 枚举
`krew-config` SHALL 定义 `AgentToAgentRouting` 枚举，包含变体：`Immediate`、`Queued`。该枚举 SHALL 派生 `Deserialize`、`Clone`、`Debug`，默认值为 `Immediate`。

#### Scenario: AgentToAgentRouting 变体
- **WHEN** 从 TOML 反序列化 `"immediate"` 或 `"queued"`
- **THEN** 各值 SHALL 映射到对应变体

#### Scenario: AgentToAgentRouting 默认值
- **WHEN** 未指定 `agent_to_agent_routing`
- **THEN** SHALL 默认为 `AgentToAgentRouting::Immediate`
