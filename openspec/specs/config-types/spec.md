## ADDED Requirements

### Requirement: Config 根结构体
`krew-config` SHALL 定义 `Config` 结构体，包含字段：`settings: Settings`、`agents: Vec<AgentConfig>`、`providers: HashMap<String, ProviderConfig>`、`mcp_servers: Vec<McpServerConfig>`、`skills: SkillsConfig`。该结构体 SHALL 派生 `Deserialize`。`skills` 字段 SHALL 使用 `Default` trait 提供默认值，当 TOML 中不存在 `[skills]` 节时自动使用默认配置。

#### Scenario: Config 结构体可导入
- **WHEN** 导入 `krew_config::Config`
- **THEN** 该类型 SHALL 可访问并包含所有指定字段，包括 `skills: SkillsConfig`

#### Scenario: skills 字段默认值
- **WHEN** TOML 配置中不包含 `[skills]` 节
- **THEN** `Config::skills` SHALL 使用 `SkillsConfig::default()`（enabled=true, extra_paths=[]）

### Requirement: Settings 结构体
`krew-config` SHALL 定义 `Settings` 结构体，包含字段：`approval_mode: ApprovalMode`、`reply_order: Vec<String>`、`auto_compact_threshold: Option<u32>`、`other_agent_role: OtherAgentRole`（默认 `User`）、`agent_to_agent_routing: AgentToAgentRouting`（默认 `Immediate`）、`agent_to_agent_max_rounds: u32`（默认 `10`）。

#### Scenario: Settings 字段齐全
- **WHEN** 构造 `Settings` 值
- **THEN** 所有字段 SHALL 存在且类型正确

#### Scenario: other_agent_role 默认值
- **WHEN** settings TOML 块未包含 `other_agent_role` 字段
- **THEN** 默认值 SHALL 为 `OtherAgentRole::User`

#### Scenario: other_agent_role 配置为 assistant
- **WHEN** settings TOML 块包含 `other_agent_role = "assistant"`
- **THEN** SHALL 反序列化为 `OtherAgentRole::Assistant`

#### Scenario: agent_to_agent_routing 默认值
- **WHEN** settings TOML 块未包含 `agent_to_agent_routing` 字段
- **THEN** 默认值 SHALL 为 `AgentToAgentRouting::Immediate`

#### Scenario: agent_to_agent_max_rounds 默认值
- **WHEN** settings TOML 块未包含 `agent_to_agent_max_rounds` 字段
- **THEN** 默认值 SHALL 为 `10`

### Requirement: AgentConfig 结构体
`krew-config` SHALL 定义 `AgentConfig` 结构体，包含字段：`name`、`display_name`、`provider`、`model`（均为 `String`）、`api_type: Option<ApiType>`、`color: String`、`system_prompt: Option<String>`、`tools: bool`、`enable_web_search: bool`、`sampling: Option<SamplingConfig>`、`enable_thinking: bool`（默认 false）、`thinking_effort: Option<ThinkingEffort>`。

#### Scenario: AgentConfig 字段映射
- **WHEN** 反序列化一个 agent TOML 块
- **THEN** 所有字段 SHALL 正确映射到结构体

#### Scenario: enable_thinking 默认 false
- **WHEN** agent TOML 块中未设置 `enable_thinking`
- **THEN** SHALL 默认为 `false`

#### Scenario: thinking_effort 可选
- **WHEN** agent TOML 块中未设置 `thinking_effort`
- **THEN** `thinking_effort` SHALL 为 `None`

### Requirement: SamplingConfig 结构体
`krew-config` SHALL 定义 `SamplingConfig` 结构体，所有字段均为可选：`temperature`、`top_p`（f64）、`top_k`、`max_tokens`（u32）、`frequency_penalty`、`presence_penalty`（f64）、`stop_sequences`（Vec<String>）。

#### Scenario: SamplingConfig 全字段可选
- **WHEN** 构造未设置任何字段的 `SamplingConfig`
- **THEN** 结构体 SHALL 有效，所有字段为 `None`

### Requirement: ProviderConfig 结构体
`krew-config` SHALL 定义 `ProviderConfig` 结构体，包含可选字段：`api_key`、`api_key_env`、`base_url`、`azure_endpoint`、`azure_api_version`（均为 `Option<String>`）、`vertex_project: Option<String>`、`vertex_location: Option<String>`。

#### Scenario: ProviderConfig 字段
- **WHEN** 反序列化 provider TOML 块
- **THEN** 所有字段正确映射，类型正确

#### Scenario: Vertex AI 字段
- **WHEN** Provider TOML 块包含 `vertex_project = "my-proj"` 和 `vertex_location = "us-central1"`
- **THEN** 正确反序列化两个字段

### Requirement: McpServerConfig 结构体
`krew-config` SHALL 定义 `McpServerConfig` 结构体，包含字段：`name: String`、`command: String`、`args: Vec<String>`、`env: Option<HashMap<String, String>>`、`trust: Option<McpTrust>`。

#### Scenario: McpServerConfig 字段映射
- **WHEN** 反序列化一个 MCP 服务器 TOML 块
- **THEN** 所有字段 SHALL 正确映射

### Requirement: ApprovalMode 枚举
`krew-config` SHALL 定义 `ApprovalMode` 枚举，包含变体：`Suggest`、`AutoEdit`、`FullAuto`。

#### Scenario: ApprovalMode 变体
- **WHEN** 从 TOML 反序列化 `"suggest"`、`"auto-edit"`、`"full-auto"`
- **THEN** 各值 SHALL 映射到对应变体

### Requirement: ApiType 枚举
`krew-config` SHALL 定义 `ApiType` 枚举，包含变体：`Responses`、`Chat`。

#### Scenario: ApiType 变体
- **WHEN** 从 TOML 反序列化 `"responses"` 或 `"chat"`
- **THEN** 各值 SHALL 映射到对应变体

### Requirement: McpTrust 枚举
`krew-config` SHALL 定义 `McpTrust` 枚举，包含变体：`Auto`、`Confirm`。

#### Scenario: McpTrust 变体
- **WHEN** 从 TOML 反序列化 `"auto"` 或 `"confirm"`
- **THEN** 各值 SHALL 映射到对应变体

### Requirement: OtherAgentRole 枚举定义
`krew-config` SHALL 定义 `OtherAgentRole` 枚举，包含变体：`User`、`Assistant`。该枚举 SHALL 派生 `Deserialize`、`Clone`、`Debug`。

#### Scenario: OtherAgentRole 变体
- **WHEN** 从 TOML 反序列化 `"user"` 或 `"assistant"`
- **THEN** 映射为对应的 `OtherAgentRole::User` 或 `OtherAgentRole::Assistant`

### Requirement: 指令文件名常量
`krew-config` SHALL 定义公开常量 `PROJECT_INSTRUCTIONS_FILENAME`，值为 `"AGENTS.md"`。

#### Scenario: 常量可访问
- **WHEN** 导入 `krew_config::PROJECT_INSTRUCTIONS_FILENAME`
- **THEN** 其值 SHALL 为 `"AGENTS.md"`

### Requirement: 指令文件大小限制常量
`krew-config` SHALL 定义公开常量 `PROJECT_INSTRUCTIONS_MAX_SIZE`，值为 `102400`（100KB）。

#### Scenario: 常量可访问
- **WHEN** 导入 `krew_config::PROJECT_INSTRUCTIONS_MAX_SIZE`
- **THEN** 其值 SHALL 为 `102400`
## Requirements
### Requirement: McpServerConfig Clone support
`McpServerConfig` SHALL derive `Clone` to allow MCP manager to clone configs during concurrent server startup.

#### Scenario: McpServerConfig can be cloned
- **WHEN** cloning a McpServerConfig instance
- **THEN** it SHALL produce an identical copy with all fields preserved

### Requirement: McpTrust default value
`McpTrust` SHALL default to `Confirm` when not specified in the MCP server config.

#### Scenario: McpTrust defaults to Confirm
- **WHEN** MCP server config does not specify a trust field
- **THEN** `trust.unwrap_or_default()` SHALL return `McpTrust::Confirm`

### Requirement: AgentToAgentRouting 枚举
`krew-config` SHALL 定义 `AgentToAgentRouting` 枚举，包含变体：`Immediate`、`Queued`。该枚举 SHALL 派生 `Deserialize`、`Clone`、`Debug`，默认值为 `Immediate`。

#### Scenario: AgentToAgentRouting 变体
- **WHEN** 从 TOML 反序列化 `"immediate"` 或 `"queued"`
- **THEN** 各值 SHALL 映射到对应变体

#### Scenario: AgentToAgentRouting 默认值
- **WHEN** 未指定 `agent_to_agent_routing`
- **THEN** SHALL 默认为 `AgentToAgentRouting::Immediate`

