## MODIFIED Requirements

### Requirement: 寻址解析与 stdin 分离
寻址解析 MUST 仅针对原始 `-p` 参数执行，MUST NOT 包含 stdin 管道内容。系统 SHALL 先从 `-p` 参数解析 `@agent` 或 `#agent` 寻址，再读取 stdin 拼接为消息 body。`#agent` 密语寻址 SHALL 使用与 TUI 相同的语义——标记 `whisper_targets` 并在 agent 间执行可见性过滤。

#### Scenario: stdin 包含 @agent token
- **WHEN** 用户运行 `echo "@gpt hello" | krew -p "@claude review"`
- **THEN** 消息仅路由到 claude，stdin 中的 `@gpt` 不影响路由

#### Scenario: 仅从 -p 参数解析寻址
- **WHEN** 用户运行 `cat code.rs | krew -p "@all analyze"`
- **THEN** 系统对 `"@all analyze"` 执行 `parse_input()` 获得 `Addressee::All`

#### Scenario: P 模式密语寻址
- **WHEN** 用户运行 `krew -p "#opus what do you think?"`
- **THEN** 系统 SHALL 解析为密语模式，消息 `whisper_targets = Some(["opus"])`，agent 间可见性过滤正常执行

#### Scenario: P 模式 #all 被拒绝
- **WHEN** 用户运行 `krew -p "#all hello"`
- **THEN** 系统 SHALL 输出错误信息并以 exit code 2 退出

### Requirement: 寻址强制要求
`-p` 模式下，prompt MUST 包含至少一个已知 `@agent`、`@all`、或 `#agent` 寻址。若 `parse_input()` 返回 `Addressee::LastRespondent`（即 prompt 中无已知 agent 的 `@` 或 `#` 前缀），系统 SHALL 报错退出（exit code 2）。

#### Scenario: 缺少寻址
- **WHEN** 用户运行 `krew -p "hello"`
- **THEN** 系统输出错误信息 "Prompt mode requires @agent, #agent, or @all addressing" 并以 exit code 2 退出

#### Scenario: 使用 #agent 寻址
- **WHEN** 用户运行 `krew -p "#opus review this"`
- **THEN** 系统正常执行密语模式，不报错

### Requirement: AI-to-AI 路由
`-p` 模式 SHALL 支持 AI-to-AI 路由。当密语模式下 agent 响应中包含 `@other_agent` mention 时，系统 SHALL 仅路由组内 Agent，组外 Agent 的 mention 被忽略。

#### Scenario: 密语模式下组内 A2A
- **WHEN** P 模式密语中 opus 的回复包含 "@gemini"，且 gemini 在 whisper_targets 中
- **THEN** 系统将 gemini 加入调度队列，gemini 的回复继承 whisper_targets

#### Scenario: 密语模式下组外 A2A 被忽略
- **WHEN** P 模式密语中 opus 的回复包含 "@gpt"，且 gpt 不在 whisper_targets 中
- **THEN** 系统忽略此 @mention
