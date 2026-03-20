## ADDED Requirements

### Requirement: /rewind 命令注册
`/rewind` SHALL 注册为内置 slash 命令，出现在命令解析、帮助列表和 tab 补全中。

#### Scenario: 命令解析
- **WHEN** 用户输入 `/rewind`
- **THEN** `SlashCommand::from_input()` SHALL 返回 `Some(SlashCommand::Rewind)`

#### Scenario: /help 列表
- **WHEN** 用户执行 `/help`
- **THEN** 输出 SHALL 包含 `/rewind` 及其描述 "Rewind to a previous message"

#### Scenario: Tab 补全
- **WHEN** 用户输入 `/rew` 并触发补全
- **THEN** `/rewind` SHALL 出现在补全候选列表中
