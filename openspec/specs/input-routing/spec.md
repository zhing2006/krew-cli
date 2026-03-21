## ADDED Requirements

### Requirement: @ 寻址集成
用户输入 SHALL 经过 `parse_input(input, known_agents)` 解析，从输入的任意位置识别 `@all`、`@<agent_name>` 和 `@name1 @name2` 寻址模式。只有匹配已配置 Agent 名称（或 `all`）的 `@token` 才被识别为寻址，未知的 `@token` 和裸 `@` 当作普通文本。消息正文 SHALL 始终保留完整原文（包含 `@name`），不剥离 `@token`。

#### Scenario: @all 广播
- **WHEN** 用户输入 `@all hello`
- **THEN** 系统 SHALL 解析为 `Addressee::All`，消息正文为 `@all hello`

#### Scenario: @name 定向（开头）
- **WHEN** 用户输入 `@gpt explain this` 且 `gpt` 在已配置 agents 中
- **THEN** 系统 SHALL 解析为 `Addressee::Single("gpt")`，消息正文为 `@gpt explain this`

#### Scenario: @name 定向（中间）
- **WHEN** 用户输入 `hey @gpt what do you think`
- **THEN** 系统 SHALL 解析为 `Addressee::Single("gpt")`，消息正文为 `hey @gpt what do you think`

#### Scenario: 多 Agent 寻址
- **WHEN** 用户输入 `@gpt @opus debate this`
- **THEN** 系统 SHALL 解析为 `Addressee::Multiple(["gpt","opus"])`，消息正文为 `@gpt @opus debate this`

#### Scenario: 多 Agent 散落在文中
- **WHEN** 用户输入 `hey @gpt what does @opus think`
- **THEN** 系统 SHALL 解析为 `Addressee::Multiple(["gpt","opus"])`，消息正文为 `hey @gpt what does @opus think`

#### Scenario: 无前缀发给上一个回答者
- **WHEN** 用户输入 `just chatting`（无 @ 前缀）
- **THEN** 系统 SHALL 解析为 `Addressee::LastRespondent`，消息正文为 `just chatting`

### Requirement: 未知 @ 当作普通文本
未知的 `@token`（不匹配任何已配置 Agent）和裸 `@` SHALL 被当作普通文本，不报错，不影响寻址解析。

#### Scenario: 未知 Agent 名称
- **WHEN** 用户输入 `@unknown hello` 且 `unknown` 不在配置的 agents 列表中
- **THEN** 系统 SHALL 解析为 `Addressee::LastRespondent`，消息正文为 `@unknown hello`

#### Scenario: 裸 @
- **WHEN** 用户输入 `@ hello`
- **THEN** 系统 SHALL 解析为 `Addressee::LastRespondent`，消息正文为 `@ hello`

#### Scenario: 已知与未知混合
- **WHEN** 用户输入 `@gpt @unknown hello`
- **THEN** 系统 SHALL 只识别 `gpt`，解析为 `Addressee::Single("gpt")`，消息正文为 `@gpt @unknown hello`

### Requirement: 用户消息路由指示
用户消息 SHALL 在 `> ` 前缀后显示彩色圆点表示消息发送目标。无目标时不显示指示符。

#### Scenario: 单 Agent 彩色点
- **WHEN** 用户发送消息给单个 Agent
- **THEN** 用户消息 SHALL 显示为 `> ● message`，圆点颜色为目标 Agent 的配置颜色

#### Scenario: 多 Agent 彩色点
- **WHEN** 用户发送消息给多个 Agent 或 @all
- **THEN** 用户消息 SHALL 显示为 `> ●●● message`，每个圆点颜色对应各 Agent 的配置颜色

#### Scenario: 无目标无指示符
- **WHEN** 用户发送无 @ 的消息（LastRespondent）
- **THEN** 用户消息 SHALL 显示为 `> message`，不带任何指示符

### Requirement: Echo 回复
当消息寻址到的 Agent 有 LLM 客户端时，SHALL 调用 agent loop 进行实际 LLM 对话。仅当 Agent 为 builtin echo 类型时，回显消息 SHALL 以黄色菱形 `◆` 前缀和路由标记显示。

#### Scenario: LLM Agent 调用
- **WHEN** 用户输入 `@gpt explain this` 且 gpt agent 有 LlmClient
- **THEN** 系统 SHALL 调用 `agent.complete(messages)` 启动 LLM 对话，不再 echo 回显

#### Scenario: builtin echo 保持
- **WHEN** 用户输入 `@echo hello` 且 echo agent 的 provider 为 "builtin"
- **THEN** echo 回显 SHALL 显示为 `◆ [→ @echo] echo: @echo hello`，菱形为黄色

#### Scenario: @all 路由标记
- **WHEN** 用户输入 `@all hello`
- **THEN** echo 回显 SHALL 显示为 `◆ [→ @all] echo: @all hello`，菱形为黄色

#### Scenario: 无前缀路由标记
- **WHEN** 用户输入 `just chatting`
- **THEN** echo 回显 SHALL 显示为 `◆ [→ last] echo: just chatting`，菱形为黄色

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

### Requirement: 消息历史管理
App SHALL 维护当前会话的消息历史列表（`Vec<ChatMessage>`），用于构建 LLM 请求的上下文。

#### Scenario: 用户消息加入历史
- **WHEN** 用户发送消息
- **THEN** SHALL 构建 `ChatMessage { role: User, content, addressee }` 并追加到消息历史

#### Scenario: Agent 回复加入历史
- **WHEN** Agent 回复完成（收到 Done 事件）
- **THEN** SHALL 构建 `ChatMessage { role: Assistant, name: agent_name, content: 累积的完整回复, usage }` 并追加到消息历史

#### Scenario: 历史传递给 Agent
- **WHEN** 调用 `agent.complete()`
- **THEN** SHALL 传入完整的消息历史列表，使 LLM 具有上下文
