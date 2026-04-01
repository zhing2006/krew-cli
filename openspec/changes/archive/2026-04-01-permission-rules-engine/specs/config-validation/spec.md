## ADDED Requirements

### Requirement: 权限规则工具名校验
`Config::validate()` SHALL 检查 `allow_rules`、`deny_rules`、`ask_rules` 中的 `tool` 字段值是否为已知工具名。已知工具名包括：`shell`、`write_file`、`edit_file`、`read_file`、`fetch_url`、`glob`、`grep`、`activate_skill`、`run_agent`，以及 `mcp_` 前缀的 MCP 工具名。

#### Scenario: 有效工具名通过校验
- **WHEN** deny_rules 包含 `tool = "shell"`
- **THEN** validate() SHALL 通过

#### Scenario: 无效工具名报错
- **WHEN** deny_rules 包含 `tool = "unknown_tool"`
- **THEN** validate() SHALL 返回 `Err(ConfigError::Validation(...))` 且错误信息包含无效工具名

#### Scenario: MCP 工具名通过校验
- **WHEN** allow_rules 包含 `tool = "mcp_github_create_issue"`
- **THEN** validate() SHALL 通过（以 `mcp_` 前缀开头）

### Requirement: Shell 通配符 pattern 语法校验
`Config::validate()` SHALL 检查 `tool = "shell"` 规则的 `pattern` 字段不包含未转义的正则特殊字符（除 `*` 外），以防止用户误用正则语法。校验失败 SHALL 返回 `Err(ConfigError::Validation(...))`。

#### Scenario: 有效通配符 pattern
- **WHEN** deny_rules 包含 `tool = "shell"`, `pattern = "rm -rf *"`
- **THEN** validate() SHALL 通过

#### Scenario: pattern 包含正则语法报错
- **WHEN** deny_rules 包含 `tool = "shell"`, `pattern = "rm -rf .+"`
- **THEN** validate() SHALL 返回 `Err(ConfigError::Validation(...))` 且错误信息提示应使用 `*` 通配符而非正则语法 `.+`
