## MODIFIED Requirements

### Requirement: Anthropic thinking 参数映射
当 `enable_thinking = true` 时，Anthropic Client SHALL 根据能力函数矩阵在请求中添加 thinking 和 effort 参数。

#### Scenario: adaptive 模型（Opus 4.6 / Sonnet 4.6）使用 adaptive thinking
- **WHEN** 模型名包含 `(opus|sonnet)` 且包含 `4-6`，且 `enable_thinking = true`
- **THEN** SHALL 设置 `"thinking": {"type": "adaptive"}`

#### Scenario: adaptive 模型带 max effort
- **WHEN** 模型满足 `supports_adaptive` 且 `thinking_effort = Some(Max)`
- **THEN** SHALL 设置 `"output_config": {"effort": "max"}`

#### Scenario: adaptive 模型带 low/medium/high effort
- **WHEN** 模型满足 `supports_adaptive` 且 `thinking_effort = Some(High)`
- **THEN** SHALL 设置 `"output_config": {"effort": "high"}`

#### Scenario: 支持 effort 但非 adaptive 的模型（Opus 4.5）使用 budget_tokens + effort
- **WHEN** 模型名包含 `opus` 且包含 `4-5`，且 `enable_thinking = true`
- **THEN** SHALL 设置 `"thinking": {"type": "enabled", "budget_tokens": <mapped>}`，同时设置 `"output_config": {"effort": "<mapped>"}`

#### Scenario: 支持 effort 模型 Max 降级
- **WHEN** 模型满足 `supports_effort` 但不满足 `supports_max_effort`，且 `thinking_effort = Some(Max)`
- **THEN** SHALL 静默降级，设置 `"output_config": {"effort": "high"}` 和 `"thinking": {"type": "enabled", "budget_tokens": 32768}`

#### Scenario: 不支持 effort 的非 adaptive 模型使用 budget_tokens 且无 effort
- **WHEN** 模型名不满足 `supports_effort` 也不满足 `supports_adaptive`（如 Sonnet 4.5, Haiku 4.5, 旧模型），且 `enable_thinking = true`
- **THEN** SHALL 设置 `"thinking": {"type": "enabled", "budget_tokens": <mapped>}`，不发送 `output_config`

#### Scenario: Legacy 模型 Max effort 降级为 budget_tokens high
- **WHEN** 模型不满足 `supports_effort` 且 `thinking_effort = Some(Max)`
- **THEN** SHALL 设置 `"thinking": {"type": "enabled", "budget_tokens": 32768}`，不发送 `output_config`（Max 在 budget 映射中等同 High）

#### Scenario: budget_tokens effort 映射
- **WHEN** 模型使用 budget_tokens 模式
- **THEN** effort 映射 SHALL 为：Low→1024, Medium→8192, High→32768, Max→32768，未设置 effort 时默认 8192

#### Scenario: thinking 启用时 temperature 强制为 1
- **WHEN** `enable_thinking = true` 且用户设置了 `temperature != 1.0`
- **THEN** SHALL 在请求中设置 `temperature: 1.0`，并通过 tracing::warn 记录覆盖行为

### Requirement: Gemini thinking Max effort 映射
当 `thinking_effort = Some(Max)` 时，Google Client SHALL 将 Max 等同 High 处理。

#### Scenario: Gemini 3.x 模型 Max effort
- **WHEN** 模型名匹配 `gemini-3*` 且 `thinking_effort = Some(Max)`
- **THEN** SHALL 设置 `"thinkingConfig": {"includeThoughts": true, "thinkingLevel": "high"}`

#### Scenario: Gemini 2.5 模型 Max effort
- **WHEN** 模型名匹配 `gemini-2.5*` 且 `thinking_effort = Some(Max)`
- **THEN** SHALL 设置 `"thinkingConfig": {"includeThoughts": true, "thinkingBudget": 24576}`

### Requirement: OpenAI Responses reasoning Max effort 映射
当 `thinking_effort = Some(Max)` 时，OpenAI Responses Client SHALL 根据模型能力判断是否发送 `"xhigh"`。

#### Scenario: 支持 xhigh 的模型 Max effort
- **WHEN** `enable_thinking = true` 且 `thinking_effort = Some(Max)` 且模型名在 xhigh 白名单中（`gpt-5.4`、`gpt-5.4-pro`、`gpt-5.3-codex`、`gpt-5.2`）
- **THEN** SHALL 设置 `"reasoning": {"effort": "xhigh", "summary": "auto"}`

#### Scenario: 不在白名单的模型 Max effort 降级
- **WHEN** `enable_thinking = true` 且 `thinking_effort = Some(Max)` 且模型名不在 xhigh 白名单中
- **THEN** SHALL 静默降级，设置 `"reasoning": {"effort": "high", "summary": "auto"}`

### Requirement: OpenAI Chat reasoning_effort Max 映射
当 `thinking_effort = Some(Max)` 时，OpenAI Chat Client SHALL 根据模型能力判断是否发送 `"xhigh"`。

#### Scenario: 支持 xhigh 的模型 Max effort
- **WHEN** `enable_thinking = true` 且 `thinking_effort = Some(Max)` 且模型名在 xhigh 白名单中
- **THEN** SHALL 设置 `"reasoning_effort": "xhigh"`

#### Scenario: 不在白名单的模型 Max effort 降级
- **WHEN** `enable_thinking = true` 且 `thinking_effort = Some(Max)` 且模型名不在 xhigh 白名单中
- **THEN** SHALL 静默降级，设置 `"reasoning_effort": "high"`
