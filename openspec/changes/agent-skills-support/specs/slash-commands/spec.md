## MODIFIED Requirements

### Requirement: 占位命令
`/skills` SHALL 不再是占位命令。`/mcp` 和 `/compact` SHALL 保持占位状态，显示 "not yet implemented" 提示。

#### Scenario: /skills 不再是占位命令
- **WHEN** 用户输入 `/skills`
- **THEN** 系统 SHALL 执行 skill 列表显示逻辑，而非显示占位提示

#### Scenario: /mcp 仍为占位
- **WHEN** 用户输入 `/mcp`
- **THEN** 系统 SHALL 显示 "not yet implemented" 提示
