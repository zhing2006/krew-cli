## ADDED Requirements

### Requirement: write_file tool handler
`krew-tools` SHALL implement a `WriteFileTool` struct implementing `ToolHandler`. The tool SHALL create or overwrite a file at the specified path with the provided content. The tool name SHALL be `"write_file"`.

#### Scenario: Create new file
- **WHEN** write_file is called with `{ "file_path": "src/utils.rs", "content": "pub fn hello() {}" }`
- **THEN** the file SHALL be created with the specified content, and ToolResult SHALL contain a success message indicating the file was created

#### Scenario: Overwrite existing file
- **WHEN** write_file is called on an existing file
- **THEN** the file SHALL be overwritten with the new content, and ToolResult SHALL indicate the file was updated

### Requirement: write_file 路径验证

WriteFileTool SHALL 根据 `restrict_workspace` 配置决定是否执行 workspace 边界检查。

#### Scenario: restrict_workspace = false 时允许写入外部文件
- **WHEN** `restrict_workspace = false` 且 file_path 指向 workspace 外
- **THEN** SHALL 正常创建/写入文件

#### Scenario: restrict_workspace = true 时拒绝外部文件
- **WHEN** `restrict_workspace = true` 且 file_path 指向 workspace 外
- **THEN** SHALL 返回 "outside the workspace boundary" 错误

### Requirement: write_file creates parent directories
write_file SHALL create any missing parent directories automatically (equivalent to `mkdir -p`).

#### Scenario: Nested path creation
- **WHEN** write_file is called with `file_path: "src/deep/nested/file.rs"` and `src/deep/nested/` does not exist
- **THEN** all parent directories SHALL be created, and the file SHALL be written successfully

### Requirement: write_file requires approval
`WriteFileTool::requires_approval()` SHALL return `true`.

#### Scenario: Approval flag
- **WHEN** checking `write_file_tool.requires_approval()`
- **THEN** SHALL return `true`

### Requirement: write_file parameter schema
write_file SHALL define its parameter schema with required field `file_path` (string) and `content` (string).

#### Scenario: Schema definition
- **WHEN** `write_file_tool.spec()` is called
- **THEN** parameters SHALL include `file_path` (required, string) and `content` (required, string)
