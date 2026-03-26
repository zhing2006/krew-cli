## MODIFIED Requirements

### Requirement: 只读工具路径验证

只读工具（glob、grep、read_file）SHALL 根据 `restrict_workspace` 配置决定是否执行 workspace 边界检查。

#### Scenario: restrict_workspace = true 时保持边界检查
- **WHEN** `restrict_workspace = true` 且工具接收到 workspace 外的路径
- **THEN** SHALL 返回错误

#### Scenario: restrict_workspace = false 时允许外部路径
- **WHEN** `restrict_workspace = false` 且工具接收到 workspace 外的路径
- **THEN** SHALL 正常执行并返回结果

### Requirement: glob 遍历时的边界过滤

glob 工具在遍历文件时 SHALL 根据 `restrict_workspace` 配置决定是否过滤 cwd 外的结果。

#### Scenario: restrict_workspace = false 时不过滤遍历结果
- **WHEN** `restrict_workspace = false`
- **THEN** glob SHALL 不执行 `cwd_canonical` 前缀过滤，返回搜索目录下所有匹配文件

### Requirement: create_readonly_registry 参数

`create_readonly_registry` SHALL 接受 `restrict_workspace: bool` 参数并传递给各只读工具。

#### Scenario: 透传 restrict_workspace 到只读工具
- **WHEN** 调用 `create_readonly_registry(cwd, restrict_workspace)`
- **THEN** 创建的 ReadFileTool、GlobTool、GrepTool SHALL 携带该 `restrict_workspace` 值
