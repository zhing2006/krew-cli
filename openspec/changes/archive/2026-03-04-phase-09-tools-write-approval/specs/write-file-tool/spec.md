## ADDED Requirements

### Requirement: write_file tool handler
`krew-tools` SHALL implement a `WriteFileTool` struct implementing `ToolHandler`. The tool SHALL create or overwrite a file at the specified path with the provided content. The tool name SHALL be `"write_file"`.

#### Scenario: Create new file
- **WHEN** write_file is called with `{ "file_path": "src/utils.rs", "content": "pub fn hello() {}" }`
- **THEN** the file SHALL be created with the specified content, and ToolResult SHALL contain a success message indicating the file was created

#### Scenario: Overwrite existing file
- **WHEN** write_file is called on an existing file
- **THEN** the file SHALL be overwritten with the new content, and ToolResult SHALL indicate the file was updated

### Requirement: write_file path boundary enforcement
write_file SHALL validate that the target path is within the session working directory using the shared `validate_path()` helper. Paths outside the boundary SHALL be rejected.

#### Scenario: Path outside boundary
- **WHEN** write_file is called with `file_path: "/etc/passwd"`
- **THEN** ToolResult SHALL contain an error message and `is_error: true`

#### Scenario: Relative path resolution
- **WHEN** write_file is called with a relative path like `"src/utils.rs"`
- **THEN** the path SHALL be resolved relative to the session working directory

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
