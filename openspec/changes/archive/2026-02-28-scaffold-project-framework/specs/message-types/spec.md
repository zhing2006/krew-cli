## ADDED Requirements

### Requirement: ChatMessage 结构体
`krew-core` SHALL 定义 `ChatMessage` 结构体，包含字段：`role: Role`、`agent_name: Option<String>`、`addressee: Option<String>`、`content: MessageContent`、`tool_calls: Option<Vec<ToolCall>>`、`tool_results: Option<Vec<ToolCallResult>>`、`usage: Option<Usage>`、`created_at: DateTime<Utc>`。

#### Scenario: ChatMessage 结构体可导入
- **WHEN** 导入 `krew_core::message::ChatMessage`
- **THEN** 该类型 SHALL 可访问并包含所有指定字段

### Requirement: Role 枚举
`krew-core` SHALL 定义 `Role` 枚举，包含变体：`System`、`User`、`Assistant`、`Tool`。

#### Scenario: Role 变体
- **WHEN** 使用 `Role` 枚举
- **THEN** 全部四个变体 SHALL 可用

### Requirement: MessageContent 枚举
`krew-core` SHALL 定义 `MessageContent` 枚举，包含变体：`Text(String)`、`Blocks(Vec<ContentBlock>)`。

#### Scenario: MessageContent 变体
- **WHEN** 构造消息内容
- **THEN** `Text` 和 `Blocks` 两个变体 SHALL 均可使用

### Requirement: ToolCall 和 ToolCallResult 结构体
`krew-core` SHALL 定义 `ToolCall`（字段：`id: String`、`name: String`、`arguments: serde_json::Value`）和 `ToolCallResult`（字段：`tool_call_id: String`、`content: String`、`is_error: bool`）。

#### Scenario: ToolCall 结构体字段
- **WHEN** 构造一个 `ToolCall`
- **THEN** 全部三个字段 SHALL 存在

### Requirement: Session 结构体
`krew-core` SHALL 定义 `Session` 结构体，包含字段：`id: String`、`cwd: PathBuf`、`agents: Vec<String>`、`messages: Vec<ChatMessage>`、`total_tokens_used: u64`、`created_at: DateTime<Utc>`、`updated_at: DateTime<Utc>`。

#### Scenario: Session 结构体可导入
- **WHEN** 导入 `krew_core::session::Session`
- **THEN** 该类型 SHALL 可访问并包含所有指定字段

### Requirement: Addressee 枚举
`krew-core` SHALL 定义 `Addressee` 枚举，包含变体：`All`、`Single(String)`、`LastRespondent`。

#### Scenario: Addressee 变体
- **WHEN** 使用 `Addressee` 枚举
- **THEN** 全部三个变体 SHALL 可用
