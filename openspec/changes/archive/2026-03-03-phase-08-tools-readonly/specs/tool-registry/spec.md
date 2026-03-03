## ADDED Requirements

### Requirement: ToolSpec 结构体
`krew-tools` SHALL 定义 `ToolSpec` 结构体，包含字段：`name: String`、`description: String`、`parameters: serde_json::Value`。此结构体用于描述工具的 JSON Schema，发送给 LLM Provider。

#### Scenario: ToolSpec 可构造
- **WHEN** 构造一个 `ToolSpec`
- **THEN** 三个字段 SHALL 均存在

#### Scenario: ToolSpec 转换为 ToolDefinition
- **WHEN** 将 `ToolSpec` 转换为 `krew_llm::ToolDefinition`
- **THEN** name、description、parameters SHALL 一一对应

### Requirement: ToolHandler trait
`krew-tools` SHALL 定义 `ToolHandler` trait（`Send + Sync`），包含方法：`fn name(&self) -> &str`、`fn requires_approval(&self) -> bool`、`async fn execute(&self, args: Value) -> Result<ToolResult, ToolError>`。

#### Scenario: ToolHandler trait 可实现
- **WHEN** 在某个 struct 上实现 `ToolHandler` trait
- **THEN** 实现 SHALL 编译通过，包含所有必需方法

### Requirement: ToolRegistry 注册表
`krew-tools` SHALL 定义 `ToolRegistry` 结构体，管理 ToolSpec 与 ToolHandler 的配对注册。提供方法：`register(spec, handler)`、`specs() -> &[ToolSpec]`、`dispatch(name, args) -> Result<ToolResult, ToolError>`。

#### Scenario: 注册工具
- **WHEN** 调用 `registry.register(spec, handler)`
- **THEN** `registry.specs()` SHALL 包含该 spec，`registry.dispatch(name, args)` SHALL 调用该 handler

#### Scenario: 分发未注册工具
- **WHEN** 调用 `registry.dispatch("unknown_tool", args)`
- **THEN** SHALL 返回 `Err(ToolError::Execution("unknown tool: unknown_tool"))`

### Requirement: 内置只读工具注册
`krew-tools` SHALL 提供 `fn create_readonly_registry(cwd: PathBuf) -> ToolRegistry` 工厂函数，注册 read_file、glob、grep 三个只读工具。

#### Scenario: 创建只读注册表
- **WHEN** 调用 `create_readonly_registry(cwd)`
- **THEN** 返回的 registry SHALL 包含 3 个 spec（read_file、glob、grep）

#### Scenario: 只读工具不需审批
- **WHEN** 检查只读注册表中任意工具的 `requires_approval()`
- **THEN** SHALL 返回 `false`
