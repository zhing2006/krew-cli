## MODIFIED Requirements

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

### Requirement: ProviderConfig 结构体
`krew-config` SHALL 定义 `ProviderConfig` 结构体，包含可选字段：`api_key`、`api_key_env`、`base_url`、`azure_endpoint`、`azure_api_version`（均为 `Option<String>`）、`use_name_field: bool`（默认 false）、`vertex_project: Option<String>`、`vertex_location: Option<String>`。

#### Scenario: ProviderConfig 字段
- **WHEN** 反序列化一个 provider TOML 块
- **THEN** 所有字段 SHALL 可选且类型正确

#### Scenario: Vertex AI 字段
- **WHEN** provider TOML 块设置 `vertex_project = "my-proj"` 和 `vertex_location = "us-central1"`
- **THEN** SHALL 正确反序列化这两个字段
