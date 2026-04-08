## MODIFIED Requirements

### Requirement: ThinkingEffort 枚举
`krew-config` SHALL 定义 `ThinkingEffort` 枚举，包含变体：`Low`、`Medium`、`High`、`Max`。SHALL 支持从 TOML 字符串 `"low"`、`"medium"`、`"high"`、`"max"` 反序列化。

#### Scenario: 反序列化 thinking effort
- **WHEN** TOML 中 `thinking_effort = "high"`
- **THEN** SHALL 反序列化为 `ThinkingEffort::High`

#### Scenario: 反序列化 max effort
- **WHEN** TOML 中 `thinking_effort = "max"`
- **THEN** SHALL 反序列化为 `ThinkingEffort::Max`

#### Scenario: 默认值
- **WHEN** `enable_thinking = true` 但未设置 `thinking_effort`
- **THEN** `thinking_effort` SHALL 为 `None`，各 Provider 使用各自默认行为
