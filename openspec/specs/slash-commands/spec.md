## ADDED Requirements

### Requirement: Slash 命令识别
以 `/` 开头的输入 SHALL 优先识别为 Slash 命令。系统 SHALL 先使用 `SlashCommand::from_input()` 检查内置命令，若无匹配再查找自定义命令注册表。仅当两者都无匹配时，SHALL 显示错误提示。

#### Scenario: 已知内置命令
- **WHEN** 用户输入 `/help`
- **THEN** 系统 SHALL 识别为内置 `SlashCommand::Help` 并执行

#### Scenario: 已知自定义命令
- **WHEN** 用户输入 `/commit fix typo` 且自定义命令 `/commit` 已注册
- **THEN** 系统 SHALL 查找自定义命令注册表，执行命令展开（参数替换 + bash 预处理），并将结果作为普通消息发送

#### Scenario: 未知命令
- **WHEN** 用户输入 `/unknown` 且内置和自定义命令均无匹配
- **THEN** 系统 SHALL 在 viewport 上方显示错误提示 `Unknown command: /unknown`

### Requirement: /help 命令
`/help` SHALL 在 viewport 上方显示所有可用命令及其描述。

#### Scenario: 显示帮助
- **WHEN** 用户输入 `/help`
- **THEN** 系统 SHALL 在 viewport 上方插入命令列表，包含每个命令的名称和描述

### Requirement: /agents 命令
`/agents` SHALL 在 viewport 上方显示当前配置的所有 Agent 信息。

#### Scenario: 显示 Agent 列表
- **WHEN** 用户输入 `/agents`
- **THEN** 系统 SHALL 显示每个 Agent 的 `[name]`（带颜色）、`display_name`、`provider/model`、token 统计（占位显示 0）

### Requirement: /new 命令
The `/new` command (also `/clear`) SHALL save the current session to disk, clear the conversation context, create a new session with a fresh UUID, clear the screen, and display the new header with the new session ID.

#### Scenario: Execute /new with active session
- **WHEN** the user runs `/new` during an active session with messages
- **THEN** the current session SHALL be saved, conversation messages and token usage SHALL be cleared, a new session id SHALL be generated, the screen SHALL be cleared, and the header SHALL show the new session id

#### Scenario: Execute /new with empty session
- **WHEN** the user runs `/new` on a session with no messages
- **THEN** the empty session SHALL NOT be saved to disk, a new session id SHALL be generated, and the screen SHALL be cleared with a new header

### Requirement: /quit 命令
`/quit` SHALL 正常退出程序。

#### Scenario: 退出
- **WHEN** 用户输入 `/quit`
- **THEN** 程序 SHALL 设置退出标志，正常退出并恢复终端

### Requirement: /stats 命令
`/stats` SHALL 在 viewport 上方显示当前进程的运行时状态信息，包括内存占用和线程数。

#### Scenario: 显示进程统计
- **WHEN** 用户输入 `/stats`
- **THEN** 系统 SHALL 在 viewport 上方显示标题行和进程统计信息，包含物理内存占用（人类可读格式）和线程数

#### Scenario: 部分数据不可用
- **WHEN** 某个统计指标在当前平台不可用
- **THEN** 该指标 SHALL 显示为 `N/A`

### Requirement: /resume command
The `/resume` command SHALL list recent sessions and allow the user to select one to resume.

#### Scenario: Execute /resume with available sessions
- **WHEN** the user runs `/resume` and there are saved sessions
- **THEN** the system SHALL display a numbered list of sessions (most recent first) showing: index, date/time, agent names, and first message preview (truncated to 40 chars)

#### Scenario: Execute /resume with no saved sessions
- **WHEN** the user runs `/resume` and there are no saved sessions
- **THEN** the system SHALL display an info message: "No saved sessions found"

#### Scenario: User selects a session to resume
- **WHEN** the user inputs a valid session number after `/resume` listing
- **THEN** the current session SHALL be saved (if non-empty), the selected session SHALL be loaded, and a confirmation message SHALL be displayed

#### Scenario: /resume shows help text
- **WHEN** the `/help` command lists available commands
- **THEN** `/resume` SHALL be described as "Resume a previous session"

### Requirement: 占位命令
`/mcp` 和 `/compact` SHALL 保持占位状态，显示 "not yet implemented" 提示。`/skills` SHALL 不再是占位命令。

#### Scenario: /skills 不再是占位命令
- **WHEN** 用户输入 `/skills`
- **THEN** 系统 SHALL 执行 skill 列表显示逻辑，而非显示占位提示

#### Scenario: /mcp 仍为占位
- **WHEN** 用户输入 `/mcp`
- **THEN** 系统 SHALL 显示 "not yet implemented" 提示

#### Scenario: /compact 占位
- **WHEN** 用户输入 `/compact`
- **THEN** 系统 SHALL 在 viewport 上方显示提示信息表明该功能待实现
