## ADDED Requirements

### Requirement: 格式保留的 TOML 配置写入
`krew-config` SHALL 提供基于 `toml_edit` 的配置文件写入能力。写入操作 SHALL 保留文件中已有的注释、空行和格式排列。

#### Scenario: 写入已有文件保留注释
- **WHEN** `~/.krew/settings.toml` 包含用户手动添加的注释
- **AND** 通过写入 API 添加新供应商
- **THEN** 原有注释 SHALL 保留在写入后的文件中

#### Scenario: 文件不存在时创建
- **WHEN** 目标配置文件不存在
- **THEN** SHALL 创建新文件并写入内容

#### Scenario: 父目录不存在时创建
- **WHEN** 目标文件的父目录（如 `.krew/`）不存在
- **THEN** SHALL 递归创建目录后写入文件

### Requirement: add_provider 写入供应商
`krew-config` SHALL 提供 `add_provider()` 函数，向配置文件追加 `[providers.<name>]` 表。

#### Scenario: 添加第一个供应商
- **WHEN** 文件中不存在 `[providers]` 节
- **THEN** SHALL 创建 `[providers.<name>]` 表并写入所有字段

#### Scenario: 追加到已有供应商
- **WHEN** 文件中已有 `[providers.anthropic]`
- **THEN** 新增 `[providers.openai]` SHALL 追加在已有供应商表之后

#### Scenario: 写入 api_key_env 字段
- **WHEN** 供应商使用环境变量方式
- **THEN** SHALL 写入 `api_key_env = "ENV_VAR_NAME"`，不写 `api_key` 字段

#### Scenario: 写入 api_key 字段
- **WHEN** 供应商使用直接存储方式
- **THEN** SHALL 写入 `api_key = "key_value"`，不写 `api_key_env` 字段

#### Scenario: 写入 base_url 字段
- **WHEN** 供应商配置了 base_url
- **THEN** SHALL 写入 `base_url = "url_value"`

#### Scenario: 写入 Vertex AI 字段
- **WHEN** 供应商配置了 vertex_project 和 vertex_location
- **THEN** SHALL 写入 `vertex_project` 和 `vertex_location` 字段

### Requirement: remove_provider 删除供应商
`krew-config` SHALL 提供 `remove_provider()` 函数，从配置文件中移除指定的 `[providers.<name>]` 表。

#### Scenario: 删除已有供应商
- **WHEN** 文件包含 `[providers.openai]` 和 `[providers.anthropic]`
- **AND** 调用 `remove_provider("openai")`
- **THEN** 文件 SHALL 仅保留 `[providers.anthropic]`

#### Scenario: 删除不存在的供应商
- **WHEN** 调用 `remove_provider("nonexistent")`
- **THEN** SHALL 返回错误，文件内容不变

### Requirement: add_agent 写入 Agent
`krew-config` SHALL 提供 `add_agent()` 函数，向配置文件追加 `[[agents]]` 表项并更新 `reply_order`。

#### Scenario: 添加第一个 Agent
- **WHEN** 文件中不存在 `[[agents]]`
- **THEN** SHALL 创建 `[settings]` 节（如不存在）和 `[[agents]]` 表项，`reply_order` 包含该 Agent 名称

#### Scenario: 追加到已有 Agent
- **WHEN** 文件中已有 2 个 `[[agents]]`
- **THEN** 新 Agent SHALL 追加在最后一个 `[[agents]]` 之后
- **AND** reply_order SHALL 在末尾追加新 Agent 名称

#### Scenario: 写入所有 Agent 字段
- **WHEN** 添加一个完整配置的 Agent
- **THEN** SHALL 写入 name、display_name、provider、model、color、enable_thinking、enable_web_search 字段
- **AND** 可选字段（api_type、system_prompt、sampling、thinking_effort）仅在有值时写入

#### Scenario: bool 字段的写入
- **WHEN** Agent 的 enable_thinking 为 true，enable_web_search 为 false
- **THEN** SHALL 写入 `enable_thinking = true` 和 `enable_web_search = false`

### Requirement: remove_agent 删除 Agent
`krew-config` SHALL 提供 `remove_agent()` 函数，从配置文件中移除指定的 `[[agents]]` 表项并更新 `reply_order`。

#### Scenario: 删除已有 Agent
- **WHEN** 文件包含名为 `gpt` 的 Agent
- **AND** 调用 `remove_agent("gpt")`
- **THEN** 该 `[[agents]]` 表项 SHALL 被移除
- **AND** `reply_order` 中的 `gpt` SHALL 被移除

#### Scenario: 删除不存在的 Agent
- **WHEN** 调用 `remove_agent("nonexistent")`
- **THEN** SHALL 返回错误，文件内容不变

### Requirement: batch_add_agents 批量写入 Agent
`krew-config` SHALL 提供 `batch_add_agents()` 函数，一次性写入多个 Agent 和完整的 reply_order。仅用于 init 预设场景（目标文件不存在或无已有 agents）。

#### Scenario: 预设写入 3 个 Agent 到新文件
- **WHEN** 调用 `batch_add_agents()` 传入 3 个 Agent 配置且目标文件不存在
- **THEN** SHALL 创建文件并写入 3 个 `[[agents]]` 表项和对应的 `reply_order`

#### Scenario: 文件已有 agents 时拒绝覆盖
- **WHEN** 调用 `batch_add_agents()` 且文件中已有 `[[agents]]`
- **THEN** SHALL 返回 `Err(ConfigError::Validation("agents already exist ..."))` 错误，不修改文件

#### Scenario: 文件存在但无 agents 时可写入
- **WHEN** 调用 `batch_add_agents()` 且文件存在但不包含 `[[agents]]`
- **THEN** SHALL 追加 agents 和 reply_order，保留文件中已有的其他配置节

### Requirement: list_providers 读取供应商列表
`krew-config` SHALL 提供 `list_providers()` 函数，从配置文件读取所有供应商名称和配置。

#### Scenario: 读取已有供应商
- **WHEN** 配置文件包含 2 个供应商
- **THEN** SHALL 返回包含 2 个 `(name, ProviderConfig)` 的列表

#### Scenario: 文件不存在
- **WHEN** 配置文件不存在
- **THEN** SHALL 返回空列表

### Requirement: list_agents 读取 Agent 列表
`krew-config` SHALL 提供 `list_agents()` 函数，从配置文件读取所有 Agent 配置和 reply_order。

#### Scenario: 读取已有 Agent
- **WHEN** 配置文件包含 3 个 Agent
- **THEN** SHALL 返回包含 3 个 `AgentConfig` 和 `reply_order: Vec<String>` 的结果

#### Scenario: 文件不存在
- **WHEN** 配置文件不存在
- **THEN** SHALL 返回空 Agent 列表和空 reply_order

### Requirement: ConfigWriter 错误类型
配置写入操作 SHALL 返回 `Result<(), ConfigError>`，复用已有的 `ConfigError` 枚举（Io / Parse / Validation 变体）。

#### Scenario: 文件写入失败
- **WHEN** 目标路径不可写（权限不足）
- **THEN** SHALL 返回 `Err(ConfigError::Io(...))`

#### Scenario: TOML 解析现有文件失败
- **WHEN** 现有配置文件格式损坏
- **THEN** SHALL 返回 `Err(ConfigError::Parse(...))`


### Requirement: add_provider writes vertex-anthropic providers
`krew-config` writer SHALL serialize `ProviderType::VertexAnthropic` as `type = "vertex-anthropic"` and SHALL write existing optional fields using the same rules as other providers.

#### Scenario: Write Vertex Anthropic provider
- **WHEN** `add_provider()` receives `ProviderWriteData` with `provider_type = ProviderType::VertexAnthropic`
- **THEN** the generated provider table SHALL contain `type = "vertex-anthropic"`

#### Scenario: Write Vertex Anthropic fields
- **WHEN** Vertex Anthropic provider data includes `api_key_env`、`base_url`、`vertex_project`、`vertex_location` and `extra_headers`
- **THEN** `add_provider()` SHALL write those fields to the provider table

#### Scenario: List Vertex Anthropic provider
- **WHEN** `list_providers()` reads a provider with `type = "vertex-anthropic"`
- **THEN** it SHALL return the provider with `ProviderType::VertexAnthropic`
