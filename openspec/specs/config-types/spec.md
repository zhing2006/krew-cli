## ADDED Requirements

### Requirement: Config 根结构体
`krew-config` SHALL 定义 `Config` 结构体，包含字段：`settings: Settings`、`agents: Vec<AgentConfig>`、`providers: HashMap<String, ProviderConfig>`、`mcp_servers: Vec<McpServerConfig>`、`skills: SkillsConfig`。该结构体 SHALL 派生 `Deserialize`。`skills` 字段 SHALL 使用 `Default` trait 提供默认值，当 TOML 中不存在 `[skills]` 节时自动使用默认配置。

#### Scenario: Config 结构体可导入
- **WHEN** 导入 `krew_config::Config`
- **THEN** 该类型 SHALL 可访问并包含所有指定字段，包括 `skills: SkillsConfig`

#### Scenario: skills 字段默认值
- **WHEN** TOML 配置中不包含 `[skills]` 节
- **THEN** `Config::skills` SHALL 使用 `SkillsConfig::default()`（enabled=true, extra_paths=[]）

### Requirement: PermissionRule 结构体定义
`krew-config` SHALL 定义 `PermissionRule` 结构体，包含字段：
- `tool: String` — 工具名（如 `"shell"`、`"write_file"`、`"read_file"`）
- `pattern: Option<String>` — 匹配模式（通配符/glob/域名，可选）
- `reason: Option<String>` — 拒绝或确认原因（可选）

该结构体 SHALL 派生 `Deserialize`、`Serialize`、`Clone`、`Debug`。

#### Scenario: 完整规则反序列化
- **WHEN** TOML 包含完整的三字段规则
- **THEN** SHALL 正确反序列化所有字段

#### Scenario: 最小规则反序列化
- **WHEN** TOML 仅包含 `tool` 字段
- **THEN** `pattern` 和 `reason` SHALL 为 `None`

### Requirement: Settings 结构体字段

`Settings` 结构体 SHALL 包含 `restrict_workspace: bool` 字段，默认值为 `true`。该字段 SHALL 可从 TOML 配置文件的 `[settings]` 段反序列化。

`Settings` 结构体 SHALL 移除以下字段：
- `shell_allow_commands: Vec<String>` — **REMOVED**
- `fetch_allow_domains: Vec<String>` — **REMOVED**

SHALL 新增以下字段：
- `allow_rules: Vec<PermissionRule>` — 自动放行规则，默认为空 Vec
- `deny_rules: Vec<PermissionRule>` — 自动拒绝规则，默认为空 Vec
- `ask_rules: Vec<PermissionRule>` — 强制确认规则，默认为空 Vec

#### Scenario: 从 TOML 反序列化 restrict_workspace
- **WHEN** 配置文件包含 `restrict_workspace = false`
- **THEN** `Settings.restrict_workspace` SHALL 为 `false`

#### Scenario: 缺省时使用默认值
- **WHEN** 配置文件中未设置 `restrict_workspace`
- **THEN** `Settings.restrict_workspace` SHALL 为 `true`

#### Scenario: 新字段反序列化
- **WHEN** TOML 包含 `[[allow_rules]]`、`[[deny_rules]]`、`[[ask_rules]]` 表
- **THEN** SHALL 正确反序列化为 `Vec<PermissionRule>`

#### Scenario: 规则字段缺省时为空
- **WHEN** TOML 中不包含任何规则相关配置
- **THEN** `allow_rules`、`deny_rules`、`ask_rules` SHALL 均为空 Vec

#### Scenario: 旧字段被忽略
- **WHEN** TOML 包含 `shell_allow_commands` 或 `fetch_allow_domains`
- **THEN** SHALL 被 serde 忽略（不影响反序列化），但通过 deprecated field 检测机制触发启动警告

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
`krew-config` SHALL 定义 `ProviderConfig` 结构体，包含必需字段 `provider_type: ProviderType`（TOML 中键名为 `type`），以及可选字段：`api_key`、`api_key_env`、`base_url`（均为 `Option<String>`）、`vertex_project: Option<String>`、`vertex_location: Option<String>`。

#### Scenario: ProviderConfig 字段
- **WHEN** 反序列化 provider TOML 块
- **THEN** 所有字段正确映射，类型正确

#### Scenario: Vertex AI 字段
- **WHEN** Provider TOML 块包含 `vertex_project = "my-proj"` 和 `vertex_location = "us-central1"`
- **THEN** 正确反序列化两个字段

### Requirement: Provider configuration fields
The `ProviderConfig` struct SHALL include an optional `extra_headers` field of type `Option<HashMap<String, String>>` that deserializes from TOML inline tables. The field SHALL default to `None` when not specified in the configuration file.

#### Scenario: Parse extra_headers from TOML
- **WHEN** a provider config contains `extra_headers = { "X-Custom" = "value" }`
- **THEN** `ProviderConfig.extra_headers` SHALL be `Some(HashMap)` containing the entry `("X-Custom", "value")`

#### Scenario: Parse config without extra_headers
- **WHEN** a provider config does not contain `extra_headers`
- **THEN** `ProviderConfig.extra_headers` SHALL be `None`

### Requirement: McpServerConfig 结构体
`krew-config` SHALL 定义 `McpServerConfig` 结构体，包含字段：`name: String`、`command: Option<String>`、`args: Vec<String>`、`env: Option<HashMap<String, String>>`、`url: Option<String>`、`headers: Option<HashMap<String, String>>`、`trust: Option<McpTrust>`。支持两种传输模式：stdio（设置 `command`）和 HTTP（设置 `url`）。

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

### Requirement: RawConfig / RawSettings / UserConfig / UserSettings 类型导出
`krew-config` SHALL 公开导出 `RawConfig`、`RawSettings`、`UserConfig`、`UserSettings` 类型，使 `krew-cli` 和 `krew-core` 可导入使用。

#### Scenario: 类型可导入
- **WHEN** 在 `krew-cli` 中 `use krew_config::{RawConfig, UserConfig}`
- **THEN** SHALL 编译成功

### Requirement: USER_CONFIG_DIR 常量
`krew-config` SHALL 定义并公开导出常量 `USER_CONFIG_DIR`，值为 `".krew"`。

#### Scenario: 常量值正确
- **WHEN** 导入 `krew_config::USER_CONFIG_DIR`
- **THEN** 其值 SHALL 为 `".krew"`

### Requirement: Deprecated field 启动警告
配置加载流程 SHALL 在 TOML 反序列化之前或之后，检测原始 TOML 文本中是否存在已废弃的字段名（`shell_allow_commands`、`fetch_allow_domains`）。如果存在，SHALL 向 `startup_warnings` 推送迁移提示消息。

#### Scenario: 检测到旧字段时显示警告
- **WHEN** settings.toml 包含 `shell_allow_commands = ["ls", "cat"]`
- **THEN** 启动时 SHALL 显示黄色警告，内容包含废弃字段名和迁移到 `[[allow_rules]]` 的提示

#### Scenario: 检测到多个旧字段时逐条警告
- **WHEN** settings.toml 同时包含 `shell_allow_commands` 和 `fetch_allow_domains`
- **THEN** 启动时 SHALL 对每个废弃字段分别显示警告

#### Scenario: 无旧字段时不警告
- **WHEN** settings.toml 不包含任何废弃字段
- **THEN** SHALL 不显示相关警告

## REMOVED Requirements

### Requirement: 默认 shell 白名单常量
**Reason**: `DEFAULT_SHELL_ALLOW_COMMANDS` 常量及 `default_shell_allow_commands()` 函数已被新的规则系统替代。
**Migration**: 用户需要将旧的 `shell_allow_commands` 转换为 `[[allow_rules]]` 格式。

