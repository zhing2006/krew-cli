## ADDED Requirements

### Requirement: Slash 命令识别
以 `/` 开头的输入 SHALL 优先识别为 Slash 命令。系统 SHALL 使用 `SlashCommand::from_input()` 解析命令。无法识别的 `/` 开头输入 SHALL 显示错误提示。

#### Scenario: 已知命令
- **WHEN** 用户输入 `/help`
- **THEN** 系统 SHALL 识别为 `SlashCommand::Help` 并执行

#### Scenario: stats 命令
- **WHEN** 用户输入 `/stats`
- **THEN** 系统 SHALL 识别为 `SlashCommand::Stats` 并执行

#### Scenario: 未知命令
- **WHEN** 用户输入 `/unknown`
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

### Requirement: /clear 命令
`/clear` SHALL 清除 viewport 上方的可见内容，不影响会话历史数据。

#### Scenario: 清屏
- **WHEN** 用户输入 `/clear`
- **THEN** 终端 SHALL 清除可见内容并重新显示头部框，输入区域保持不变

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

### Requirement: 占位命令
`/new`、`/resume`、`/compact` SHALL 在 viewport 上方显示"功能待实现"提示。

#### Scenario: /new 占位
- **WHEN** 用户输入 `/new`
- **THEN** 系统 SHALL 在 viewport 上方显示提示信息表明该功能待实现

#### Scenario: /resume 占位
- **WHEN** 用户输入 `/resume`
- **THEN** 系统 SHALL 在 viewport 上方显示提示信息表明该功能待实现

#### Scenario: /compact 占位
- **WHEN** 用户输入 `/compact`
- **THEN** 系统 SHALL 在 viewport 上方显示提示信息表明该功能待实现
