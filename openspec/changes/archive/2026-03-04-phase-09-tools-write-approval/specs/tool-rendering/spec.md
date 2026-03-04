## MODIFIED Requirements

### Requirement: 工具调用 TUI 渲染
TUI 层在收到 `AgentEvent::ToolCallStart` 和 `AgentEvent::ToolCallDone` 时 SHALL 渲染工具调用信息。对于需审批的工具，`ToolCallStart` 后 SHALL 显示审批 overlay 而非直接渲染完成。

#### Scenario: 只读工具调用开始
- **WHEN** 收到 `AgentEvent::ToolCallStart { name: "read_file", arguments: "{\"file_path\":\"src/main.rs\"}" }`
- **THEN** TUI SHALL 渲染一行：`⚡ read_file("src/main.rs")`

#### Scenario: 工具调用完成
- **WHEN** 收到 `AgentEvent::ToolCallDone { name: "read_file", result_summary: "42 lines" }`
- **THEN** TUI SHALL 渲染：`⎿ 42 lines`

#### Scenario: 写工具审批后显示
- **WHEN** 收到 ApprovalRequest for edit_file，用户 approve 后收到 ToolCallDone
- **THEN** TUI SHALL 先显示审批 overlay → 用户决定 → 显示 ToolCallDone 结果

## ADDED Requirements

### Requirement: 写工具 diff 渲染
TUI SHALL 使用 diff 渲染模块显示 edit_file 的修改预览。diff 渲染 SHALL 使用 GitHub 风格着色（绿色插入、红色删除）并支持语法高亮。

#### Scenario: edit_file diff 显示
- **WHEN** edit_file 审批 overlay 包含 unified diff
- **THEN** TUI SHALL 渲染带颜色的 diff（+ 行绿色背景，- 行红色背景）

#### Scenario: write_file 内容预览
- **WHEN** write_file 审批 overlay 显示
- **THEN** TUI SHALL 渲染文件内容预览（前 N 行）

### Requirement: shell 输出渲染
shell 工具完成后，TUI SHALL 渲染命令输出。输出 SHALL 保留原始格式，不做 markdown 解析。

#### Scenario: shell 输出显示
- **WHEN** shell 执行 `cargo test` 完成
- **THEN** ToolCallDone result_summary SHALL 显示退出码和输出摘要
