## MODIFIED Requirements

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
