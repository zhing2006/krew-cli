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

### Requirement: ToolRegistry 查询工具审批状态
`ToolRegistry` SHALL 提供 `fn requires_approval(&self, name: &str) -> bool` 方法，查询指定工具是否需要审批。

#### Scenario: 查询已注册工具
- **WHEN** 调用 `registry.requires_approval("shell")`
- **THEN** SHALL 返回 `true`

#### Scenario: 查询未注册工具
- **WHEN** 调用 `registry.requires_approval("unknown")`
- **THEN** SHALL 返回 `false`（安全默认）
## Requirements
### Requirement: Dynamic MCP tool registration
ToolRegistry SHALL support dynamic registration of MCP tools after initial creation via `register()`. MCP tools are registered during MCP server startup.

#### Scenario: Dynamic registration of MCP tools
- **WHEN** MCP server discovers 3 tools and calls `registry.register()` 3 times
- **THEN** registry SHALL contain 10+ specs total (7 built-in + activate_skill if applicable + 3 MCP)

### Requirement: MCP tool approval query
ToolRegistry SHALL correctly report approval status for registered MCP tools via `requires_approval()`.

#### Scenario: Query MCP tool with trust=auto
- **WHEN** registered an MCP tool with trust=auto and calling `registry.requires_approval("mcp__fs__read")`
- **THEN** SHALL return `false`

#### Scenario: Query MCP tool with trust=confirm and destructive
- **WHEN** registered an MCP tool with trust=confirm and destructive_hint=true and calling `registry.requires_approval("mcp__fs__delete")`
- **THEN** SHALL return `true`

