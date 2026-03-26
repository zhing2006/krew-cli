## ADDED Requirements

### Requirement: edit_file tool handler
`krew-tools` SHALL implement an `EditFileTool` struct implementing `ToolHandler`. The tool SHALL perform search-and-replace editing on an existing file. The tool name SHALL be `"edit_file"`.

#### Scenario: Simple replacement
- **WHEN** edit_file is called with `{ "file_path": "src/main.rs", "old_string": "println!(\"hello\")", "new_string": "println!(\"world\")" }`
- **THEN** the first occurrence of `old_string` in the file SHALL be replaced with `new_string`

#### Scenario: old_string not found
- **WHEN** edit_file is called and `old_string` does not exist in the file
- **THEN** ToolResult SHALL contain an error message and `is_error: true`

#### Scenario: File does not exist
- **WHEN** edit_file is called on a non-existent file
- **THEN** ToolResult SHALL contain an error message and `is_error: true`

### Requirement: edit_file generates unified diff
edit_file SHALL generate a unified diff (using the `similar` crate) comparing the original file content with the modified content. This diff SHALL be included in the ToolResult for display.

#### Scenario: Diff generation
- **WHEN** edit_file successfully replaces text
- **THEN** ToolResult SHALL contain a unified diff showing the changes with context lines

### Requirement: edit_file 路径验证

EditFileTool SHALL 根据 `restrict_workspace` 配置决定是否执行 workspace 边界检查。

#### Scenario: restrict_workspace = false 时允许编辑外部文件
- **WHEN** `restrict_workspace = false` 且 file_path 指向 workspace 外
- **THEN** SHALL 正常执行编辑操作

#### Scenario: restrict_workspace = true 时拒绝外部文件
- **WHEN** `restrict_workspace = true` 且 file_path 指向 workspace 外
- **THEN** SHALL 返回 "outside the workspace boundary" 错误

### Requirement: edit_file requires approval
`EditFileTool::requires_approval()` SHALL return `true`.

#### Scenario: Approval flag
- **WHEN** checking `edit_file_tool.requires_approval()`
- **THEN** SHALL return `true`

### Requirement: edit_file parameter schema
edit_file SHALL define its parameter schema with required fields `file_path` (string), `old_string` (string), and `new_string` (string).

#### Scenario: Schema definition
- **WHEN** `edit_file_tool.spec()` is called
- **THEN** parameters SHALL include `file_path`, `old_string`, `new_string` (all required strings)

### Requirement: edit_file unique match
edit_file SHALL verify that `old_string` appears exactly once in the file. If it appears multiple times, the tool SHALL return an error suggesting the user provide a more specific `old_string` with more surrounding context.

#### Scenario: Multiple matches
- **WHEN** edit_file is called and `old_string` appears 3 times in the file
- **THEN** ToolResult SHALL contain an error explaining the match is ambiguous and `is_error: true`
