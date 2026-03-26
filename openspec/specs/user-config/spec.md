## ADDED Requirements

### Requirement: RawConfig 结构体
`krew-config` SHALL 定义 `RawConfig` 结构体，包含字段：`settings: RawSettings`（默认 `Default`）、`agents: Vec<AgentConfig>`、`providers: HashMap<String, ProviderConfig>`（默认空）、`mcp_servers: Vec<McpServerConfig>`（默认空）、`skills: Option<SkillsConfig>`（默认 `None`）。该结构体 SHALL 派生 `Deserialize`、`Clone`、`Debug`。

#### Scenario: RawConfig 反序列化完整配置
- **WHEN** 反序列化包含 `[settings]`、`[[agents]]`、`[providers.*]`、`[[mcp_servers]]` 的 TOML 文件
- **THEN** SHALL 返回 `RawConfig`，所有字段正确填充

#### Scenario: RawConfig settings 字段保留存在性
- **WHEN** TOML 中 `[settings]` 只包含 `approval_mode = "full-auto"`
- **THEN** `raw.settings.approval_mode` SHALL 为 `Some(FullAuto)`，其余标量字段 SHALL 为 `None`

### Requirement: RawSettings 结构体
`krew-config` SHALL 定义 `RawSettings` 结构体，所有标量字段均为 `Option`：`approval_mode`、`auto_compact_threshold`、`compact_keep_rounds`、`input_history_limit`、`paste_burst_detection`、`worker_threads`、`other_agent_role`、`retry`、`shell_allow_commands`、`fetch_allow_domains`、`agent_to_agent_routing`、`agent_to_agent_max_rounds`、`language`、`restrict_workspace`。`reply_order` SHALL 为 `Vec<String>`（非 Option，project-only）。该结构体 SHALL 派生 `Deserialize`、`Clone`、`Debug`、`Default`。

#### Scenario: RawSettings 部分字段设置
- **WHEN** TOML 中 `[settings]` 包含 `approval_mode = "full-auto"` 和 `restrict_workspace = false`
- **THEN** 这两个字段 SHALL 为 `Some`，其余 SHALL 为 `None`

#### Scenario: RawSettings reply_order 默认空
- **WHEN** TOML 中 `[settings]` 未设置 `reply_order`
- **THEN** `reply_order` SHALL 为空 `Vec`

### Requirement: UserConfig 结构体
`krew-config` SHALL 定义 `UserConfig` 结构体，包含字段：`settings: UserSettings`（默认 `Default`）、`providers: HashMap<String, ProviderConfig>`（默认空）、`mcp_servers: Vec<McpServerConfig>`（默认空）、`skills: Option<SkillsConfig>`（默认 `None`）。该结构体 SHALL 派生 `Deserialize`、`Clone`、`Debug`、`Default`。`UserConfig` SHALL 不包含 `agents` 和 `reply_order` 字段。

#### Scenario: UserConfig 反序列化完整配置
- **WHEN** 反序列化包含 `[settings]`、`[providers.*]`、`[[mcp_servers]]` 的 TOML 文件
- **THEN** SHALL 返回 `UserConfig`，所有字段正确填充

#### Scenario: UserConfig 反序列化空文件
- **WHEN** 反序列化空 TOML 内容
- **THEN** SHALL 返回 `UserConfig::default()`，所有字段为空/None

### Requirement: UserSettings 结构体
`krew-config` SHALL 定义 `UserSettings` 结构体，字段与 `RawSettings` 相同但不含 `reply_order`，所有字段均为 `Option`（包括 `restrict_workspace: Option<bool>`）。该结构体 SHALL 派生 `Deserialize`、`Clone`、`Debug`、`Default`。

#### Scenario: UserSettings 设置 restrict_workspace
- **WHEN** user config TOML 中 `[settings]` 包含 `restrict_workspace = false`
- **THEN** SHALL 反序列化为 `UserSettings`，`restrict_workspace` 为 `Some(false)`

### Requirement: UserConfig 加载
`krew-config` SHALL 提供 `UserConfig::load() -> UserConfig` 静态方法，从 `~/.krew/settings.toml` 加载 user 级配置。

#### Scenario: User config 文件存在
- **WHEN** `~/.krew/settings.toml` 存在且内容合法
- **THEN** SHALL 返回解析后的 `UserConfig`

#### Scenario: User config 文件不存在
- **WHEN** `~/.krew/settings.toml` 不存在
- **THEN** SHALL 返回 `UserConfig::default()`，不输出任何提示

#### Scenario: User config 解析失败
- **WHEN** `~/.krew/settings.toml` 存在但 TOML 格式错误
- **THEN** SHALL 通过 `eprintln!` 输出终端可见的 warning 消息（包含文件路径和错误原因），并返回 `UserConfig::default()`

### Requirement: RawConfig 合并 UserConfig
`RawConfig` SHALL 提供 `pub fn merge_user(&mut self, user: &UserConfig)` 方法，在 resolve 之前将 user 级配置合并进来。合并规则如下：

- **`providers`**: user 的 providers 作为 base，project 的同名 key 整项替换 user 的
- **`mcp_servers`**: user 的 MCP servers 在前，project 的追加在后；同名 server（by name）中 project 的替换 user 的
- **`settings` 标量字段**: project `Some` 优先；project `None` 时使用 user 的值（包括 `restrict_workspace`）
- **`skills`**: project `Some` 时用 project 的；project `None` 时用 user 的

#### Scenario: providers 合并——project 覆盖
- **WHEN** user config 定义 `providers.openai`，project config 也定义 `providers.openai`
- **THEN** 合并后 SHALL 使用 project 的 `providers.openai`（整项替换）

#### Scenario: providers 合并——user 补充
- **WHEN** user config 定义 `providers.anthropic`，project config 未定义
- **THEN** 合并后 SHALL 包含 user 的 `providers.anthropic`

#### Scenario: mcp_servers 合并去重
- **WHEN** user config 定义 name="context7"，project config 也定义同名 server
- **THEN** 合并后 SHALL 只保留 project 的 "context7" 定义

#### Scenario: mcp_servers 追加
- **WHEN** user config 定义 name="global-mcp"，project config 中无此名
- **THEN** 合并后 SHALL 包含 "global-mcp"，且位于 project mcp_servers 之前

#### Scenario: settings 标量——project 优先
- **WHEN** user config 设置 `approval_mode = "full-auto"`，project config 设置 `approval_mode = "suggest"`
- **THEN** 合并后 `approval_mode` SHALL 为 `Some(Suggest)`

#### Scenario: settings 标量——user 补充
- **WHEN** user config 设置 `worker_threads = 8`，project config 未设置（`None`）
- **THEN** 合并后 `worker_threads` SHALL 为 `Some(8)`

#### Scenario: settings 标量——双方均未设置
- **WHEN** user config 和 project config 均未设置 `worker_threads`
- **THEN** 合并后 `worker_threads` SHALL 为 `None`（由 resolve 填充默认值）

#### Scenario: settings restrict_workspace——project 优先
- **WHEN** user config 设置 `restrict_workspace = false`，project config 设置 `restrict_workspace = true`
- **THEN** 合并后 `restrict_workspace` SHALL 为 `Some(true)`

#### Scenario: settings restrict_workspace——user 补充
- **WHEN** user config 设置 `restrict_workspace = false`，project config 未设置（`None`）
- **THEN** 合并后 `restrict_workspace` SHALL 为 `Some(false)`

#### Scenario: settings restrict_workspace——双方均未设置
- **WHEN** user config 和 project config 均未设置 `restrict_workspace`
- **THEN** 合并后 `restrict_workspace` SHALL 为 `None`（由 resolve 填充默认值 `true`）

#### Scenario: skills 合并——project 优先
- **WHEN** project config 定义 `[skills]`，user config 也定义 `[skills]`
- **THEN** 合并后 SHALL 使用 project 的 `skills` 配置

#### Scenario: skills 合并——user 补充
- **WHEN** project config 未定义 `[skills]`，user config 定义 `[skills]`
- **THEN** 合并后 SHALL 使用 user 的 `skills` 配置

### Requirement: RawConfig resolve
`RawConfig` SHALL 提供 `pub fn resolve(self) -> Config` 方法，将所有 `Option` 字段填充为默认值，返回最终 `Config`。

#### Scenario: 所有字段有值
- **WHEN** 合并后 `RawConfig` 所有 settings 字段为 `Some`
- **THEN** resolve SHALL 使用这些值构造 `Config`

#### Scenario: 字段为 None
- **WHEN** 合并后 `approval_mode` 为 `None`
- **THEN** resolve SHALL 使用 `ApprovalMode::default()`（即 `Suggest`）

#### Scenario: restrict_workspace 为 None
- **WHEN** 合并后 `restrict_workspace` 为 `None`
- **THEN** resolve SHALL 使用默认值 `true`

#### Scenario: restrict_workspace 为 Some(false)
- **WHEN** 合并后 `restrict_workspace` 为 `Some(false)`
- **THEN** resolve SHALL 使用 `false`

#### Scenario: skills 为 None
- **WHEN** 合并后 `skills` 为 `None`
- **THEN** resolve SHALL 使用 `SkillsConfig::default()`

### Requirement: User Config 路径常量
`krew-config` SHALL 定义公开常量 `USER_CONFIG_DIR`，值为 `".krew"`。User config 文件路径 SHALL 为 `<home>/<USER_CONFIG_DIR>/settings.toml`。

#### Scenario: 常量可访问
- **WHEN** 导入 `krew_config::USER_CONFIG_DIR`
- **THEN** 其值 SHALL 为 `".krew"`
