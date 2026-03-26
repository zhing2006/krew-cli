## MODIFIED Requirements

### Requirement: 内置只读工具注册
`krew-tools` SHALL 提供 `fn create_readonly_registry(cwd: PathBuf, restrict_workspace: bool) -> ToolRegistry` 工厂函数，注册 read_file、glob、grep 三个只读工具，并将 `restrict_workspace` 透传给各工具构造函数。

#### Scenario: 创建只读注册表
- **WHEN** 调用 `create_readonly_registry(cwd, true)`
- **THEN** 返回的 registry SHALL 包含 3 个 spec（read_file、glob、grep）

### Requirement: 完整工具注册
`krew-tools` SHALL 提供 `fn create_full_registry(cwd: PathBuf, restrict_workspace: bool, skills: HashMap<String, SkillInfo>) -> ToolRegistry` 工厂函数，注册所有 7 个内置工具：read_file、glob、grep、write_file、edit_file、shell、fetch_url，以及 activate_skill（当 skills 非空时）。`restrict_workspace` SHALL 透传给 5 个文件工具（read_file、glob、grep、write_file、edit_file），不影响 shell 和 fetch_url。

#### Scenario: 创建完整注册表
- **WHEN** 调用 `create_full_registry(cwd, false, skills)`
- **THEN** 返回的 registry SHALL 包含 7 个 spec（无 skills 时）或 8 个 spec（有 skills 时）

#### Scenario: 写工具需审批
- **WHEN** 检查完整注册表中 write_file、edit_file、shell 的 `requires_approval()`
- **THEN** 三者均 SHALL 返回 `true`

#### Scenario: 只读工具不需审批
- **WHEN** 检查完整注册表中 read_file、glob、grep 的 `requires_approval()`
- **THEN** 三者均 SHALL 返回 `false`
