## MODIFIED Requirements

### Requirement: Tool trait
`krew-tools` SHALL 保留现有 `Tool` trait 作为 `ToolHandler` trait 的别名或废弃。新的工具系统 SHALL 使用 `ToolHandler` trait（定义在 `tool-registry` capability 中）和 `ToolSpec` 结构体。原有 `Tool` trait 中的 `name()`、`description()`、`parameters_schema()` 职责 SHALL 由 `ToolSpec` 承担，`requires_approval()` 和 `execute()` 职责 SHALL 由 `ToolHandler` 承担。

#### Scenario: ToolHandler trait 可实现
- **WHEN** 在某个 struct 上实现 `ToolHandler` trait
- **THEN** 实现 SHALL 编译通过，包含 `name()`、`requires_approval()`、`execute()` 方法

### Requirement: ToolResult 结构体
`krew-tools` SHALL 定义 `ToolResult` 结构体，包含字段：`content: String`、`is_error: bool`。

#### Scenario: ToolResult 结构体字段
- **WHEN** 构造一个 `ToolResult`
- **THEN** 两个字段 SHALL 均存在

### Requirement: 内置工具模块文件
`krew-tools` SHALL 包含 `builtin/` 目录，其中有各内置工具的模块文件：`read_file.rs`、`write_file.rs`、`edit_file.rs`、`shell.rs`、`glob.rs`、`grep.rs`，以及 `mod.rs`。Phase 8 中 `read_file.rs`、`glob.rs`、`grep.rs` SHALL 包含完整实现，其余 MAY 仅包含占位代码。

#### Scenario: 只读工具模块实现
- **WHEN** 构建 `krew-tools`
- **THEN** read_file、glob、grep 模块 SHALL 包含完整的 `ToolHandler` 实现

### Requirement: MCP 模块文件
`krew-tools` SHALL 包含 `mcp.rs` 模块文件，MAY 仅包含占位代码。

#### Scenario: MCP 模块编译通过
- **WHEN** 构建 `krew-tools`
- **THEN** `mcp` 模块 SHALL 被包含在编译中
