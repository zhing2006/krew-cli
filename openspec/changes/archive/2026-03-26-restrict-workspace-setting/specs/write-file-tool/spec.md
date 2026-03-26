## MODIFIED Requirements

### Requirement: write_file 路径验证

WriteFileTool SHALL 根据 `restrict_workspace` 配置决定是否执行 workspace 边界检查。

#### Scenario: restrict_workspace = false 时允许写入外部文件
- **WHEN** `restrict_workspace = false` 且 file_path 指向 workspace 外
- **THEN** SHALL 正常创建/写入文件

#### Scenario: restrict_workspace = true 时拒绝外部文件
- **WHEN** `restrict_workspace = true` 且 file_path 指向 workspace 外
- **THEN** SHALL 返回 "outside the workspace boundary" 错误
