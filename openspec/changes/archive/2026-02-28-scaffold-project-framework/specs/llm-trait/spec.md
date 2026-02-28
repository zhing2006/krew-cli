## ADDED Requirements

### Requirement: LlmClient trait
`krew-llm` SHALL 定义 `LlmClient` trait，包含一个异步方法：`chat_stream(&self, messages: &[ChatMessage], tools: &[ToolDefinition], sampling: &SamplingConfig) -> Result<Pin<Box<dyn Stream<Item = StreamEvent>>>>`。该 trait SHALL 要求 `Send + Sync`。

#### Scenario: LlmClient trait 可作为 trait object
- **WHEN** 使用 `Box<dyn LlmClient>`
- **THEN** trait object SHALL 编译通过无错误

### Requirement: StreamEvent 枚举
`krew-llm` SHALL 定义 `StreamEvent` 枚举，包含变体：`TextDelta(String)`、`ToolCall { id: String, name: String, arguments: String }`、`ThinkingDelta(String)`、`Done(Usage)`、`Error(String)`。

#### Scenario: StreamEvent 变体
- **WHEN** 对 `StreamEvent` 进行模式匹配
- **THEN** 全部五个变体 SHALL 可匹配

### Requirement: Usage 结构体
`krew-llm` SHALL 定义 `Usage` 结构体，包含字段：`prompt_tokens: u32`、`completion_tokens: u32`、`total_tokens: u32`。

#### Scenario: Usage 结构体字段
- **WHEN** 构造一个 `Usage` 值
- **THEN** 全部三个字段 SHALL 存在

### Requirement: Provider 模块文件
`krew-llm` SHALL 包含各 provider 的源文件：`openai_responses.rs`、`openai_chat.rs`、`openai_compatible.rs`、`anthropic.rs`、`google.rs`。每个文件 SHALL 作为模块存在，MAY 仅包含占位代码。

#### Scenario: Provider 模块编译通过
- **WHEN** 构建 `krew-llm`
- **THEN** 全部五个 provider 模块 SHALL 被包含在编译中

### Requirement: OtherAgentRole 枚举
`krew-llm` SHALL 定义 `OtherAgentRole` 枚举，包含变体：`User`、`Assistant`。

#### Scenario: OtherAgentRole 变体
- **WHEN** 使用该枚举
- **THEN** 两个变体 SHALL 均可用
