## ADDED Requirements

### Requirement: # 密语寻址解析
`parse_input()` SHALL 识别 `#name` token 作为密语寻址，使用与 `@name` 相同的 token 扫描和匹配逻辑。`#name` 仅匹配已配置的 Agent 名称。返回值 SHALL 包含 `is_whisper: bool` 标志，当检测到任何 `#` 寻址时为 `true`。`#` 和 `@` 不可混用——输入中同时出现 `#name` 和 `@name` 时，SHALL 报错。

#### Scenario: 单目标密语
- **WHEN** 用户输入 `#opus hello` 且 `opus` 在已配置 agents 中
- **THEN** 系统 SHALL 解析为 `Addressee::Single("opus")`，`is_whisper = true`，消息正文为 `#opus hello`

#### Scenario: 多目标密语
- **WHEN** 用户输入 `#opus #gemini discuss this`
- **THEN** 系统 SHALL 解析为 `Addressee::Multiple(["opus", "gemini"])`，`is_whisper = true`

#### Scenario: #all 被拒绝
- **WHEN** 用户输入 `#all hello`
- **THEN** 系统 SHALL 返回错误，提示 `#all` 不被支持

#### Scenario: 未知 #name 当作普通文本
- **WHEN** 用户输入 `#unknown hello` 且 `unknown` 不在已配置 agents 中
- **THEN** 系统 SHALL 解析为 `Addressee::LastRespondent`，`is_whisper = false`

#### Scenario: # 和 @ 混合使用被拒绝
- **WHEN** 用户输入 `#opus @gemini hello`
- **THEN** 系统 SHALL 返回错误，提示不能混合使用 `#` 和 `@`

#### Scenario: #name 在句中
- **WHEN** 用户输入 `hey #opus what do you think`
- **THEN** 系统 SHALL 解析为 `Addressee::Single("opus")`，`is_whisper = true`
