## ADDED Requirements

### Requirement: 工具调用 TUI 渲染
TUI 层在收到 `AgentEvent::ToolCallStart` 和 `AgentEvent::ToolCallDone` 时 SHALL 渲染工具调用信息。

#### Scenario: 工具调用开始
- **WHEN** 收到 `AgentEvent::ToolCallStart { name: "read_file", arguments: "{\"file_path\":\"src/main.rs\"}" }`
- **THEN** TUI SHALL 渲染一行：`⚡ read_file("src/main.rs")`，参数从 JSON 中提取主要参数值

#### Scenario: 工具调用完成
- **WHEN** 收到 `AgentEvent::ToolCallDone { name: "read_file", result_summary: "42 lines" }`
- **THEN** TUI SHALL 将该行更新为：`⚡ read_file("src/main.rs") — 42 lines`

#### Scenario: grep 结果摘要
- **WHEN** grep 工具返回 5 个匹配结果
- **THEN** result_summary SHALL 为 `"5 matches"`

#### Scenario: glob 结果摘要
- **WHEN** glob 工具返回 12 个匹配文件
- **THEN** result_summary SHALL 为 `"12 files"`

### Requirement: 工具调用行样式
工具调用行 SHALL 使用 dimmed/gray 样式（与普通文本区分），`⚡` 符号 SHALL 使用亮色（如 yellow）。

#### Scenario: 工具行视觉区分
- **WHEN** TUI 渲染工具调用行
- **THEN** SHALL 与普通 Markdown 文本行有明显视觉区分

### Requirement: 工具调用参数简化显示
工具调用渲染 SHALL 仅显示主要参数值，不显示参数名和完整 JSON。

#### Scenario: read_file 显示
- **WHEN** arguments 为 `{"file_path": "src/main.rs", "offset": 10, "limit": 5}`
- **THEN** SHALL 显示 `⚡ read_file("src/main.rs", offset=10, limit=5)`

#### Scenario: grep 显示
- **WHEN** arguments 为 `{"pattern": "TODO", "include": "*.rs"}`
- **THEN** SHALL 显示 `⚡ grep("TODO", include="*.rs")`
