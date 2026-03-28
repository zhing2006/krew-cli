### Requirement: /tools 命令显示非 MCP runtime tools
`/tools` SHALL 在 viewport 上方按 agent 分组显示每个 agent 可用的非 MCP runtime tools。使用 `is_mcp_tool()` 负向过滤排除 MCP 工具（`mcp__` 前缀），保留其余所有 runtime tools。

#### Scenario: 显示有工具的 agent
- **WHEN** 用户输入 `/tools` 且 agent "gpt" 已成功初始化，注册了 8 个 built-in 工具和 2 个 MCP 工具
- **THEN** 系统 SHALL 显示 `[gpt]  GPT ─── 8 tool(s)` header 行，后跟 8 个非 MCP 工具的名称和描述，MCP 工具不出现

#### Scenario: 显示 tools=false 的 agent
- **WHEN** 用户输入 `/tools` 且 agent "reader" 已成功初始化但配置了 `tools=false`（registry 为空）
- **THEN** 系统 SHALL 显示 `[reader]  Reader ─── no tool(s)` header 行，不展开工具列表

#### Scenario: 显示初始化失败的 agent
- **WHEN** 用户输入 `/tools` 且 agent "broken" 因 provider 或 API key 问题未能初始化（不在 `self.agents` 中）
- **THEN** 系统 SHALL 显示 `[broken]  Broken ─── unavailable` header 行，不展开工具列表

#### Scenario: 显示 sub-agent 工具
- **WHEN** 用户输入 `/tools` 且 agent 注册了 `run_agent` 工具
- **THEN** `run_agent` SHALL 出现在该 agent 的工具列表中，与其他非 MCP 工具一同显示

### Requirement: /tools 工具列表格式
每个工具 SHALL 以 4 空格缩进显示，工具名称左对齐，描述以 DarkGray 色跟随。

#### Scenario: 工具行格式
- **WHEN** 工具 `read_file` 的描述为 "Read file contents"
- **THEN** 该行 SHALL 渲染为 4 空格缩进 + 名称（左对齐 16 字符宽）+ 灰色描述文本

### Requirement: /tools agent 遍历顺序
`/tools` SHALL 按 `config.agents` 中的声明顺序遍历 agent，与 `/agents` 命令保持一致。

#### Scenario: agent 顺序
- **WHEN** 配置文件中 agent 顺序为 [gpt, opus, reader]
- **THEN** `/tools` 输出 SHALL 按 gpt → opus → reader 顺序显示

### Requirement: /tools 命令注册
`/tools` SHALL 注册为内置 slash 命令，出现在命令解析、帮助列表和 tab 补全中。

#### Scenario: 命令解析
- **WHEN** 用户输入 `/tools`
- **THEN** `SlashCommand::from_input()` SHALL 返回 `Some(SlashCommand::Tools)`

#### Scenario: /help 列表
- **WHEN** 用户执行 `/help`
- **THEN** 输出 SHALL 包含 `/tools` 及其描述

#### Scenario: Tab 补全
- **WHEN** 用户输入 `/too` 并触发补全
- **THEN** `/tools` SHALL 出现在补全候选列表中
