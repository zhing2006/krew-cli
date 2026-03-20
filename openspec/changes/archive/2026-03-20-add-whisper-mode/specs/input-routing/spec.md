## MODIFIED Requirements

### Requirement: @ 寻址集成
用户输入 SHALL 经过 `parse_input(input, known_agents)` 解析，从输入的任意位置识别 `@all`、`@<agent_name>`、`@name1 @name2` 寻址模式，以及 `#<agent_name>`、`#name1 #name2` 密语寻址模式。只有匹配已配置 Agent 名称（或 `all`）的 `@token`/`#token` 才被识别为寻址，未知的 token 和裸 `@`/`#` 当作普通文本。`#all` SHALL 被拒绝并返回错误。`#` 和 `@` 不可在同一输入中混用。消息正文 SHALL 始终保留完整原文，不剥离 token。返回值 SHALL 包含 `is_whisper: bool` 标志。

#### Scenario: @all 广播
- **WHEN** 用户输入 `@all hello`
- **THEN** 系统 SHALL 解析为 `Addressee::All`，`is_whisper = false`，消息正文为 `@all hello`

#### Scenario: @name 定向（开头）
- **WHEN** 用户输入 `@gpt explain this` 且 `gpt` 在已配置 agents 中
- **THEN** 系统 SHALL 解析为 `Addressee::Single("gpt")`，`is_whisper = false`，消息正文为 `@gpt explain this`

#### Scenario: @name 定向（中间）
- **WHEN** 用户输入 `hey @gpt what do you think`
- **THEN** 系统 SHALL 解析为 `Addressee::Single("gpt")`，`is_whisper = false`，消息正文为 `hey @gpt what do you think`

#### Scenario: 多 Agent 寻址
- **WHEN** 用户输入 `@gpt @opus debate this`
- **THEN** 系统 SHALL 解析为 `Addressee::Multiple(["gpt","opus"])`，`is_whisper = false`，消息正文为 `@gpt @opus debate this`

#### Scenario: 多 Agent 散落在文中
- **WHEN** 用户输入 `hey @gpt what does @opus think`
- **THEN** 系统 SHALL 解析为 `Addressee::Multiple(["gpt","opus"])`，`is_whisper = false`，消息正文为 `hey @gpt what does @opus think`

#### Scenario: 无前缀发给上一个回答者
- **WHEN** 用户输入 `just chatting`（无 @ 或 # 前缀）
- **THEN** 系统 SHALL 解析为 `Addressee::LastRespondent`，`is_whisper = false`，消息正文为 `just chatting`

#### Scenario: 单目标密语
- **WHEN** 用户输入 `#opus hello` 且 `opus` 在已配置 agents 中
- **THEN** 系统 SHALL 解析为 `Addressee::Single("opus")`，`is_whisper = true`，消息正文为 `#opus hello`

#### Scenario: 多目标密语
- **WHEN** 用户输入 `#opus #gemini discuss this`
- **THEN** 系统 SHALL 解析为 `Addressee::Multiple(["opus", "gemini"])`，`is_whisper = true`

#### Scenario: #all 被拒绝
- **WHEN** 用户输入 `#all hello`
- **THEN** 系统 SHALL 返回错误

#### Scenario: # 和 @ 混合被拒绝
- **WHEN** 用户输入 `#opus @gemini hello`
- **THEN** 系统 SHALL 返回错误

### Requirement: 用户消息路由指示
用户消息 SHALL 在 `> ` 前缀后显示路由指示符表示消息发送目标。普通消息使用彩色圆点，密语消息额外在圆点前显示锁图标。

#### Scenario: 单 Agent 彩色点
- **WHEN** 用户发送普通消息给单个 Agent
- **THEN** 用户消息 SHALL 显示为 `> ● message`，圆点颜色为目标 Agent 的配置颜色

#### Scenario: 多 Agent 彩色点
- **WHEN** 用户发送普通消息给多个 Agent 或 @all
- **THEN** 用户消息 SHALL 显示为 `> ●●● message`，每个圆点颜色对应各 Agent 的配置颜色

#### Scenario: 无目标无指示符
- **WHEN** 用户发送无 @ 的消息（LastRespondent）
- **THEN** 用户消息 SHALL 显示为 `> message`，不带任何指示符

#### Scenario: 密语消息锁图标
- **WHEN** 用户发送密语消息
- **THEN** 用户消息 SHALL 显示为 `> 🔒● message` 或 `> 🔒●● message`，锁图标在圆点之前
