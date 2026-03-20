## ADDED Requirements

### Requirement: activate_skill tool 注册
`krew-tools` SHALL 提供 `ActivateSkillTool` 结构体，实现 `ToolHandler` trait。当系统发现了可用 skills 时，`activate_skill` 工具 SHALL 被注册到 `ToolRegistry` 中。该工具的 `requires_approval()` SHALL 返回 `false`（只读工具）。

#### Scenario: 有 skills 时注册
- **WHEN** 系统启动时发现了可用 skills
- **THEN** `ToolRegistry` SHALL 包含 `activate_skill` 工具

#### Scenario: 无 skills 时不注册
- **WHEN** 系统启动时未发现任何 skill
- **THEN** `ToolRegistry` SHALL 不包含 `activate_skill` 工具

#### Scenario: 自动执行
- **WHEN** LLM 调用 `activate_skill` 工具
- **THEN** 系统 SHALL 自动执行，与 `read_file`、`glob`、`grep` 等只读工具相同的审批策略
