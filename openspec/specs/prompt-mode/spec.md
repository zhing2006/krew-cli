## ADDED Requirements

### Requirement: CLI 参数解析
系统 SHALL 支持 `-p <prompt>` 参数进入非交互式 prompt 模式。系统 SHALL 支持 `--format <text|json>` 参数控制输出格式，默认为 `text`。`-p` 与 `--resume` MUST NOT 同时使用，同时指定时报错退出（exit code 2）。

#### Scenario: 使用 -p 参数执行 prompt
- **WHEN** 用户运行 `krew -p "@claude hello"`
- **THEN** 系统进入 prompt 模式，向 claude agent 发送包含 `@claude hello` 的消息，输出响应后退出

#### Scenario: 指定 JSON 输出格式
- **WHEN** 用户运行 `krew -p "@all hello" --format json`
- **THEN** 系统以 JSONL 格式输出每个 agent 的响应

#### Scenario: -p 与 --resume 冲突
- **WHEN** 用户运行 `krew -p "hello" --resume`
- **THEN** 系统输出错误信息并以 exit code 2 退出

### Requirement: 寻址解析与 stdin 分离
寻址解析 MUST 仅针对原始 `-p` 参数执行，MUST NOT 包含 stdin 管道内容。系统 SHALL 先从 `-p` 参数解析 `@agent` 寻址，再读取 stdin 拼接为消息 body。stdin 中出现的 `@agent` token MUST NOT 影响路由目标。消息 body SHALL 保留完整原始 prompt 文本（含 `@agent` token，不剥离），与 `parse_input()` 现有语义一致。

#### Scenario: stdin 包含 @agent token
- **WHEN** 用户运行 `echo "@gpt hello" | krew -p "@claude review"`
- **THEN** 消息仅路由到 claude，stdin 中的 `@gpt` 不影响路由；消息 body 为 `<stdin>@gpt hello</stdin>\n\n@claude review`

#### Scenario: 仅从 -p 参数解析寻址
- **WHEN** 用户运行 `cat code.rs | krew -p "@all analyze"`
- **THEN** 系统对 `"@all analyze"` 执行 `parse_input()` 获得 `Addressee::All`，消息 body 为 `<stdin>{code.rs content}</stdin>\n\n@all analyze`

### Requirement: 寻址强制要求
`-p` 模式下，prompt MUST 包含至少一个已知 `@agent` 或 `@all` 寻址。若 `parse_input()` 返回 `Addressee::LastRespondent`（即 prompt 中无已知 agent 的 `@` 前缀），系统 SHALL 报错退出（exit code 2）。未知 `@token`（如 `@nonexistent`、`@dataclass`）由 `parse_input()` 按普通文本处理，不触发特殊校验。

#### Scenario: 缺少 @ 寻址
- **WHEN** 用户运行 `krew -p "hello"`
- **THEN** 系统输出错误信息 "Prompt mode requires @agent or @all addressing" 并以 exit code 2 退出

#### Scenario: 仅包含未知 @token
- **WHEN** 用户运行 `krew -p "@nonexistent hello"`
- **THEN** `parse_input()` 将 `@nonexistent` 视为普通文本，返回 `LastRespondent`，系统输出 "Prompt mode requires @agent or @all addressing" 并以 exit code 2 退出

#### Scenario: 已知 @agent 混合未知 @token
- **WHEN** 用户运行 `krew -p "@claude explain @dataclass"`
- **THEN** 系统正常路由到 claude，`@dataclass` 作为普通文本保留在消息 body 中

#### Scenario: 使用 @all 寻址
- **WHEN** 用户运行 `krew -p "@all hello"`
- **THEN** 系统按 `reply_order` 顺序向所有 agent 发送消息

#### Scenario: 使用多个 @agent 寻址
- **WHEN** 用户运行 `krew -p "@claude @gpt hello"`
- **THEN** 系统按 @ 出现顺序向指定 agent 发送消息

### Requirement: stdin 管道输入
当 stdin 不是终端（即有管道输入）时，系统 SHALL 读取全部 stdin 内容，以 `<stdin>...</stdin>` XML 标签包裹，拼接到原始 prompt 前方作为消息 body。

#### Scenario: 管道输入文件内容
- **WHEN** 用户运行 `cat file.rs | krew -p "@claude review this"`
- **THEN** 消息 body 为 `<stdin>{file.rs content}</stdin>\n\n@claude review this`

#### Scenario: 无管道输入
- **WHEN** 用户在终端直接运行 `krew -p "@claude hello"`
- **THEN** 系统不读取 stdin，消息 body 为 `@claude hello`

### Requirement: Text 格式 streaming 输出
默认 text 格式下，系统 SHALL streaming 输出：每收到 `TextDelta` 事件立即打印并 flush stdout。每个 agent 响应前 SHALL 输出 `[agent_name]` header 行。工具调用 SHALL 以 `⚡ tool_name(args)` 格式输出，工具完成以 `   ⎿  summary` 格式输出。`ThinkingDelta` SHALL 静默丢弃。Server tool SHALL 以 `🌐 tool_name` / `   ⎿  "query"` 格式输出。Shell/MCP 工具的 `ToolCallOutput` SHALL 以 4 空格缩进输出。

#### Scenario: 单 agent streaming text 输出
- **WHEN** claude agent 发送 TextDelta "hello" 然后 TextDelta " world" 然后 Done
- **THEN** stdout 逐步输出 `[claude]\nhello world\n`，delta 到达时立即可见

#### Scenario: 多 agent text 输出
- **WHEN** claude 和 gpt 按顺序完成响应
- **THEN** stdout 先输出 `[claude]\n{claude_response}\n\n`，再输出 `[gpt]\n{gpt_response}\n`

#### Scenario: 工具调用 text 输出
- **WHEN** agent 调用 read_file 工具后响应
- **THEN** stdout 输出 `[agent]\n⚡ read_file(path)\n   ⎿  done\n\n{response_text}\n`

#### Scenario: Server tool text 输出
- **WHEN** agent 使用 web_search server tool
- **THEN** stdout 输出 `🌐 web_search\n   ⎿  "query"\n`

#### Scenario: Shell 输出 text 输出
- **WHEN** agent 调用 shell 工具并产生 ToolCallOutput
- **THEN** 每行 ToolCallOutput 以 4 空格缩进输出到 stdout

#### Scenario: ThinkingDelta 不输出
- **WHEN** agent 发送 ThinkingDelta 事件
- **THEN** 内容不输出到 stdout

### Requirement: JSON 格式非 streaming 输出
`--format json` 时，系统 SHALL 以 JSONL（每行一个 JSON 对象）格式输出，非 streaming（缓存 TextDelta，在 Done 时输出完整文本）。事件类型包括：`tool_start`、`tool_output`、`tool_done`、`server_tool_start`、`server_tool_done`、`text`。每个 JSON 对象 MUST 包含 `agent` 字段。

#### Scenario: JSON 格式单 agent 输出
- **WHEN** claude agent 完成响应
- **THEN** stdout 输出一行 `{"agent":"claude","type":"text","content":"..."}`

#### Scenario: JSON 格式包含工具调用
- **WHEN** agent 调用工具后响应
- **THEN** stdout 按顺序输出 tool_start、tool_done、text 三行 JSONL

#### Scenario: JSON 格式包含 server tool
- **WHEN** agent 使用 web_search
- **THEN** stdout 输出 `{"agent":"...","type":"server_tool_start","tool":"web_search"}` 和 `{"agent":"...","type":"server_tool_done","tool":"web_search","query":"..."}`

#### Scenario: JSON 格式包含 tool output
- **WHEN** agent 的 shell 工具产生输出
- **THEN** stdout 输出 `{"agent":"...","type":"tool_output","text":"..."}`

### Requirement: 工具调用全自动审批
`-p` 模式下，系统 SHALL 强制使用 `FullAuto` 审批模式，忽略配置文件中的 `approval_mode` 设置。对 `ApprovalRequest` 事件 SHALL 自动回复 `Approved`。

#### Scenario: 写入工具自动执行
- **WHEN** agent 请求执行 shell 命令
- **THEN** 系统自动批准执行，不等待用户确认

### Requirement: AI-to-AI 路由
`-p` 模式 SHALL 支持 AI-to-AI 路由。当 agent 响应中包含 `@other_agent` mention 时，系统按配置的路由策略（immediate/queued）将目标 agent 加入调度队列，遵循 `agent_to_agent_max_rounds` 限制。

#### Scenario: agent 回复中 @mention 另一个 agent
- **WHEN** claude 的回复中包含 "@gpt"
- **THEN** 系统将 gpt 加入调度队列，gpt 完成后输出其响应

#### Scenario: 达到 AI-to-AI 轮次上限
- **WHEN** AI-to-AI 对话轮次达到 `agent_to_agent_max_rounds`
- **THEN** 系统停止处理新的 @mention，不再添加 agent 到队列

### Requirement: 多 Agent 错误处理
当某个 agent 出错时，系统 SHALL 继续执行队列中的下一个 agent（与 TUI 模式一致）。错误信息 SHALL 输出到 stderr。最终 exit code：全部成功为 0，任一出错为 1。

#### Scenario: @all 场景部分 agent 出错
- **WHEN** `@all` 发送给 3 个 agent，其中 1 个返回 API 错误
- **THEN** 其余 2 个 agent 正常输出响应，错误信息到 stderr，exit code 1

#### Scenario: 全部 agent 出错
- **WHEN** 所有 agent 都返回错误
- **THEN** 所有错误信息输出到 stderr，exit code 1

### Requirement: Retrying 事件处理
`-p` 模式下，`Retrying` 事件 SHALL 输出到 stderr，格式为 "Retrying ({attempt}/{max_attempts}, {reason}, {delay}s)..."。

#### Scenario: agent 遇到速率限制
- **WHEN** agent 收到 429 并进入重试
- **THEN** stderr 输出重试信息，重试成功后正常输出响应到 stdout

### Requirement: Session 持久化
`-p` 模式 SHALL 像 TUI 模式一样保存 session 到 `.krew/sessions/`。session 包含用户消息、所有 agent 响应（含工具调用中间消息）和 token 用量。

#### Scenario: prompt 执行后保存 session
- **WHEN** 所有 agent 完成响应
- **THEN** 系统将完整对话保存为 session 文件，可通过 `--resume` 在 TUI 模式恢复

### Requirement: Exit code
系统 SHALL 使用以下 exit code：0 表示所有 agent 成功完成，1 表示任一 agent 出错（如 API 错误），2 表示参数解析或配置错误（如缺少已知 @ 寻址、参数冲突）。`main()` SHALL 使用 `std::process::exit()` 确保精确的 exit code 控制。

#### Scenario: 全部成功
- **WHEN** 所有 agent 成功完成响应
- **THEN** 进程以 exit code 0 退出

#### Scenario: agent 出错
- **WHEN** 某个 agent 返回 API 错误
- **THEN** 系统将错误信息输出到 stderr，进程以 exit code 1 退出

#### Scenario: 缺少寻址
- **WHEN** prompt 中无已知 agent 的 @ 寻址（如 `krew -p "hello"` 或 `krew -p "@nonexistent hello"`）
- **THEN** 系统输出 "Prompt mode requires @agent or @all addressing"，进程以 exit code 2 退出

#### Scenario: 参数冲突
- **WHEN** 用户同时指定 `-p` 和 `--resume`
- **THEN** 系统输出错误信息，进程以 exit code 2 退出

### Requirement: MCP 初始化
若配置文件中包含 MCP servers，`-p` 模式 SHALL 在执行前初始化 MCP 连接并将工具注册到 agent registries，执行完成后清理 MCP 进程。

#### Scenario: 配置了 MCP server
- **WHEN** config 包含 MCP server 配置
- **THEN** 系统启动 MCP server、注册工具，agent 可使用 MCP 工具，执行完成后关闭 MCP server
