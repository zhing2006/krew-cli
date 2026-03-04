## MODIFIED Requirements

### Requirement: 内置只读工具注册
`krew-tools` SHALL 提供 `fn create_readonly_registry(cwd: PathBuf) -> ToolRegistry` 工厂函数，注册 read_file、glob、grep 三个只读工具。此函数保持不变。

#### Scenario: 创建只读注册表
- **WHEN** 调用 `create_readonly_registry(cwd)`
- **THEN** 返回的 registry SHALL 包含 3 个 spec（read_file、glob、grep）

## ADDED Requirements

### Requirement: 完整工具注册
`krew-tools` SHALL 提供 `fn create_full_registry(cwd: PathBuf) -> ToolRegistry` 工厂函数，注册所有 6 个内置工具：read_file、glob、grep、write_file、edit_file、shell。

#### Scenario: 创建完整注册表
- **WHEN** 调用 `create_full_registry(cwd)`
- **THEN** 返回的 registry SHALL 包含 6 个 spec

#### Scenario: 写工具需审批
- **WHEN** 检查完整注册表中 write_file、edit_file、shell 的 `requires_approval()`
- **THEN** 三者均 SHALL 返回 `true`

#### Scenario: 只读工具不需审批
- **WHEN** 检查完整注册表中 read_file、glob、grep 的 `requires_approval()`
- **THEN** 三者均 SHALL 返回 `false`

### Requirement: ToolRegistry 查询工具审批状态
`ToolRegistry` SHALL 提供 `fn requires_approval(&self, name: &str) -> bool` 方法，查询指定工具是否需要审批。

#### Scenario: 查询已注册工具
- **WHEN** 调用 `registry.requires_approval("shell")`
- **THEN** SHALL 返回 `true`

#### Scenario: 查询未注册工具
- **WHEN** 调用 `registry.requires_approval("unknown")`
- **THEN** SHALL 返回 `false`（安全默认）
