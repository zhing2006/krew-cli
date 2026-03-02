## MODIFIED Requirements

### Requirement: ProviderConfig 结构体
`krew-config` SHALL 定义 `ProviderConfig` 结构体，包含可选字段：`api_key`、`api_key_env`、`base_url`、`azure_endpoint`、`azure_api_version`（均为 `Option<String>`）、`vertex_project: Option<String>`、`vertex_location: Option<String>`。

#### Scenario: ProviderConfig 字段
- **WHEN** 反序列化 provider TOML 块
- **THEN** 所有字段正确映射，类型正确

#### Scenario: Vertex AI 字段
- **WHEN** Provider TOML 块包含 `vertex_project = "my-proj"` 和 `vertex_location = "us-central1"`
- **THEN** 正确反序列化两个字段

### Requirement: Settings 结构体
`krew-config` SHALL 定义 `Settings` 结构体，包含字段：`approval_mode: ApprovalMode`、`reply_order: Vec<String>`、`auto_compact_threshold: Option<u32>`、`other_agent_role: OtherAgentRole`（默认 `User`）。

#### Scenario: Settings 字段齐全
- **WHEN** 构造 `Settings` 值
- **THEN** 所有字段存在且类型正确

#### Scenario: other_agent_role 默认值
- **WHEN** settings TOML 块未包含 `other_agent_role` 字段
- **THEN** 默认值为 `OtherAgentRole::User`

#### Scenario: other_agent_role 配置为 assistant
- **WHEN** settings TOML 块包含 `other_agent_role = "assistant"`
- **THEN** 反序列化为 `OtherAgentRole::Assistant`

## ADDED Requirements

### Requirement: OtherAgentRole 枚举定义
`krew-config` SHALL 定义 `OtherAgentRole` 枚举，包含变体：`User`、`Assistant`。该枚举 SHALL 派生 `Deserialize`、`Clone`、`Debug`。

#### Scenario: OtherAgentRole 变体
- **WHEN** 从 TOML 反序列化 `"user"` 或 `"assistant"`
- **THEN** 映射为对应的 `OtherAgentRole::User` 或 `OtherAgentRole::Assistant`

## REMOVED Requirements

### Requirement: use_name_field 字段
**Reason**: 统一使用 `[agent_name]` content 前缀方式标识 other-agent 消息，`name` 字段方式在消息合并场景下无法正确归属。
**Migration**: 从 `ProviderConfig` 中删除 `use_name_field: bool` 字段。现有 settings.toml 中的 `use_name_field` 字段将被 serde 忽略（未启用 `deny_unknown_fields`）。
