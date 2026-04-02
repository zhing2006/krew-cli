## MODIFIED Requirements

### Requirement: 已实现的命令
`/compact`、`/mcp`、`/skills`、`/tools`、`/dream` SHALL 执行各自的功能逻辑。`/compact` 的完整行为定义在 compact spec 中，`/mcp` 列出已连接的 MCP 服务器及其工具，`/skills` 列出可用的 skill 列表，`/tools` 按 agent 分组列出非 MCP runtime tools，`/dream` 的完整行为定义在 dream-command spec 中。

#### Scenario: /skills 执行 skill 列表显示
- **WHEN** 用户输入 `/skills`
- **THEN** 系统 SHALL 执行 skill 列表显示逻辑

#### Scenario: /mcp 列出 MCP 服务器
- **WHEN** 用户输入 `/mcp`
- **THEN** 系统 SHALL 列出已连接的 MCP 服务器及其提供的工具

#### Scenario: /compact 执行压缩
- **WHEN** 用户输入 `/compact`
- **THEN** 系统 SHALL 执行会话压缩逻辑（详见 compact spec）

#### Scenario: /compact 占位
- **WHEN** 用户输入 `/compact`
- **THEN** 系统 SHALL 在 viewport 上方显示提示信息表明该功能待实现

#### Scenario: /tools 列出非 MCP runtime tools
- **WHEN** 用户输入 `/tools`
- **THEN** 系统 SHALL 按 agent 分组列出每个 agent 的非 MCP runtime tools（详见 tools-command spec）

#### Scenario: /dream 执行记忆整理
- **WHEN** 用户输入 `/dream global @opus`
- **THEN** 系统 SHALL 执行记忆整理逻辑（详见 dream-command spec）
