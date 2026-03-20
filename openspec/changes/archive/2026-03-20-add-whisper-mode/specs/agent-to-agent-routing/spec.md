## MODIFIED Requirements

### Requirement: System prompt Agent 列表注入
当 AI-to-AI 路由功能启用时，系统 SHALL 在每个 Agent 的 system prompt 中注入当前会话中其他已初始化 Agent 的名称列表和 @ 用法说明。提示词 SHALL 明确要求：(1) `@name` 前后需要空格；(2) 仅在需要对方回复时才使用 `@`，提及 Agent 名字时不加 `@`。密语模式下的 system prompt 注入分为两层：隐私上下文层始终注入，@mention 协作层仅在 A2A 启用时注入。

#### Scenario: 启用时注入（普通模式）
- **WHEN** `agent_to_agent_max_rounds` > 0
- **AND** agent 不在密语模式
- **THEN** Agent 的 system prompt SHALL 包含所有已初始化 Agent 的列表及协作说明

#### Scenario: 密语模式 + A2A 启用
- **WHEN** `agent_to_agent_max_rounds` > 0
- **AND** agent 在密语模式，whisper_targets = ["opus", "gemini"]
- **AND** 当前 agent 为 "opus"
- **THEN** Agent 的 system prompt SHALL 包含两层内容：(1) 隐私上下文——说明这是私密对话、组外 agent 无法看到内容；(2) @mention 协作——仅列出 "gemini" 为可 @mention 的 Agent，不列出组外 Agent

#### Scenario: 密语模式 + A2A 关闭
- **WHEN** `agent_to_agent_max_rounds` 为 0
- **AND** agent 在密语模式，whisper_targets = ["opus"]
- **THEN** Agent 的 system prompt SHALL 仅包含隐私上下文层——说明这是私密对话、组外 agent 无法看到内容。SHALL NOT 包含任何 @mention 协作说明

#### Scenario: 密语模式单目标 + A2A 启用
- **WHEN** `agent_to_agent_max_rounds` > 0
- **AND** agent 在密语模式，whisper_targets = ["opus"]（仅一个目标）
- **AND** 当前 agent 为 "opus"
- **THEN** Agent 的 system prompt SHALL 包含隐私上下文层，但 @mention 协作层无可列出的组内 peer，SHALL 省略 @mention 说明

#### Scenario: 禁用时不注入（非密语）
- **WHEN** `agent_to_agent_max_rounds` 为 0
- **AND** agent 不在密语模式
- **THEN** Agent 的 system prompt SHALL 不包含任何 AI-to-AI 或密语相关指引

## ADDED Requirements

### Requirement: 密语模式 A2A mention 过滤
当 agent 回复属于密语对时，`parse_agent_mentions` 的结果 SHALL 被过滤为仅包含 `whisper_targets` 中的 Agent。组外 Agent 的 @mention SHALL 被静默忽略，不加入调度队列。

#### Scenario: 组内 @mention 被路由
- **WHEN** agent "opus" 在密语模式（whisper_targets = ["opus", "gemini"]）回复中 @gemini
- **THEN** 系统 SHALL 将 "gemini" 加入调度队列

#### Scenario: 组外 @mention 被忽略
- **WHEN** agent "opus" 在密语模式（whisper_targets = ["opus", "gemini"]）回复中 @gpt
- **THEN** 系统 SHALL 忽略此 @mention，不将 "gpt" 加入调度队列

#### Scenario: A2A 路由继承 whisper_targets
- **WHEN** 密语模式下 A2A 触发的 agent 开始执行
- **THEN** 该 agent 的回复 SHALL 继承相同的 `whisper_targets`
