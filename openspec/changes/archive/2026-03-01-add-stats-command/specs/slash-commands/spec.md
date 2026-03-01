## MODIFIED Requirements

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

## ADDED Requirements

### Requirement: /stats 命令
`/stats` SHALL 在 viewport 上方显示当前进程的运行时状态信息，包括内存占用和线程数。

#### Scenario: 显示进程统计
- **WHEN** 用户输入 `/stats`
- **THEN** 系统 SHALL 在 viewport 上方显示标题行和进程统计信息，包含物理内存占用（人类可读格式）和线程数

#### Scenario: 部分数据不可用
- **WHEN** 某个统计指标在当前平台不可用
- **THEN** 该指标 SHALL 显示为 `N/A`
