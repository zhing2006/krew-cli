## ADDED Requirements

### Requirement: ThinkingEffort 枚举
`krew-config` SHALL 定义 `ThinkingEffort` 枚举，包含变体：`Low`、`Medium`、`High`。SHALL 支持从 TOML 字符串 `"low"`、`"medium"`、`"high"` 反序列化。

#### Scenario: 反序列化 thinking effort
- **WHEN** TOML 中 `thinking_effort = "high"`
- **THEN** SHALL 反序列化为 `ThinkingEffort::High`

#### Scenario: 默认值
- **WHEN** `enable_thinking = true` 但未设置 `thinking_effort`
- **THEN** `thinking_effort` SHALL 为 `None`，各 Provider 使用各自默认行为

### Requirement: AgentConfig thinking 字段
`AgentConfig` SHALL 新增 `enable_thinking: bool`（默认 false）和 `thinking_effort: Option<ThinkingEffort>` 两个字段。

#### Scenario: enable_thinking 默认 false
- **WHEN** agent TOML 块中未设置 `enable_thinking`
- **THEN** SHALL 默认为 `false`

#### Scenario: 同时设置 thinking 和 effort
- **WHEN** agent TOML 块设置 `enable_thinking = true` 和 `thinking_effort = "high"`
- **THEN** `AgentConfig` SHALL 正确反序列化两个字段

### Requirement: Anthropic thinking 参数映射
当 `enable_thinking = true` 时，Anthropic Client SHALL 在请求中添加 thinking 参数。

#### Scenario: Opus 4.6 / Sonnet 4.6 使用 adaptive thinking
- **WHEN** 模型名包含 `opus-4-6` 或 `sonnet-4-6` 且 `enable_thinking = true`
- **THEN** SHALL 设置 `"thinking": {"type": "adaptive"}`，若有 effort 则同时设置 `"output_config": {"effort": "<mapped>"}`

#### Scenario: 旧模型使用 budget_tokens
- **WHEN** 模型名不含 `opus-4-6` 或 `sonnet-4-6` 且 `enable_thinking = true`
- **THEN** SHALL 设置 `"thinking": {"type": "enabled", "budget_tokens": <mapped>}`，effort 映射：low→1024, medium→8192, high→32768，未设置 effort 时默认 8192

#### Scenario: thinking 启用时 temperature 强制为 1
- **WHEN** `enable_thinking = true` 且用户设置了 `temperature != 1.0`
- **THEN** SHALL 在请求中设置 `temperature: 1.0`，并通过 tracing::warn 记录覆盖行为

### Requirement: Gemini thinking 参数映射
当 `enable_thinking = true` 时，Google Client SHALL 在 `generationConfig` 中添加 `thinkingConfig`。Gemini 3.x 模型使用 `thinkingLevel` 枚举，Gemini 2.5 模型使用 `thinkingBudget` 数值，两者不可同时设置。

#### Scenario: Gemini 3.x 模型带 effort
- **WHEN** 模型名匹配 `gemini-3*`（如 gemini-3.1-pro-preview、gemini-3-flash-preview）且 `enable_thinking = true` 且 `thinking_effort = Some(High)`
- **THEN** SHALL 设置 `"thinkingConfig": {"includeThoughts": true, "thinkingLevel": "high"}`，effort 映射：low→"low", medium→"medium", high→"high"

#### Scenario: Gemini 3.x 模型无 effort
- **WHEN** 模型名匹配 `gemini-3*` 且 `enable_thinking = true` 且 `thinking_effort = None`
- **THEN** SHALL 设置 `"thinkingConfig": {"includeThoughts": true, "thinkingLevel": "high"}`（默认 high）

#### Scenario: Gemini 2.5 模型带 effort
- **WHEN** 模型名匹配 `gemini-2.5*` 且 `enable_thinking = true` 且 `thinking_effort = Some(High)`
- **THEN** SHALL 设置 `"thinkingConfig": {"includeThoughts": true, "thinkingBudget": 24576}`，effort 映射：low→1024, medium→8192, high→24576

#### Scenario: Gemini 2.5 模型无 effort
- **WHEN** 模型名匹配 `gemini-2.5*` 且 `enable_thinking = true` 且 `thinking_effort = None`
- **THEN** SHALL 设置 `"thinkingConfig": {"includeThoughts": true, "thinkingBudget": -1}`（-1 表示动态）

#### Scenario: 未知 Gemini 模型默认使用 thinkingLevel
- **WHEN** 模型名不匹配已知模式 且 `enable_thinking = true`
- **THEN** SHALL 默认使用 `thinkingLevel` 方式（面向未来新模型）

### Requirement: OpenAI Responses reasoning 参数映射
当 `enable_thinking = true` 时，OpenAI Responses Client SHALL 在请求中添加 `reasoning` 参数。

#### Scenario: 带 effort 的 reasoning
- **WHEN** `enable_thinking = true` 且 `thinking_effort = Some(High)`
- **THEN** SHALL 设置 `"reasoning": {"effort": "high", "summary": "auto"}`

#### Scenario: 无 effort 的 reasoning
- **WHEN** `enable_thinking = true` 且 `thinking_effort = None`
- **THEN** SHALL 设置 `"reasoning": {"effort": "medium", "summary": "auto"}`（默认 medium）
