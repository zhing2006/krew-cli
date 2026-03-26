## MODIFIED Requirements

### Requirement: RawSettings 结构体
`krew-config` SHALL 定义 `RawSettings` 结构体，所有标量字段均为 `Option`：`approval_mode`、`auto_compact_threshold`、`compact_keep_rounds`、`input_history_limit`、`paste_burst_detection`、`worker_threads`、`other_agent_role`、`retry`、`shell_allow_commands`、`fetch_allow_domains`、`agent_to_agent_routing`、`agent_to_agent_max_rounds`、`language`、`restrict_workspace`。`reply_order` SHALL 为 `Vec<String>`（非 Option，project-only）。该结构体 SHALL 派生 `Deserialize`、`Clone`、`Debug`、`Default`。

#### Scenario: RawSettings 部分字段设置
- **WHEN** TOML 中 `[settings]` 包含 `approval_mode = "full-auto"` 和 `restrict_workspace = false`
- **THEN** 这两个字段 SHALL 为 `Some`，其余 SHALL 为 `None`

#### Scenario: RawSettings reply_order 默认空
- **WHEN** TOML 中 `[settings]` 未设置 `reply_order`
- **THEN** `reply_order` SHALL 为空 `Vec`

### Requirement: UserSettings 结构体
`krew-config` SHALL 定义 `UserSettings` 结构体，字段与 `RawSettings` 相同但不含 `reply_order`，所有字段均为 `Option`（包括 `restrict_workspace: Option<bool>`）。该结构体 SHALL 派生 `Deserialize`、`Clone`、`Debug`、`Default`。

#### Scenario: UserSettings 设置 restrict_workspace
- **WHEN** user config TOML 中 `[settings]` 包含 `restrict_workspace = false`
- **THEN** SHALL 反序列化为 `UserSettings`，`restrict_workspace` 为 `Some(false)`

### Requirement: RawConfig 合并 UserConfig
`RawConfig` SHALL 提供 `pub fn merge_user(&mut self, user: &UserConfig)` 方法，在 resolve 之前将 user 级配置合并进来。合并规则如下：

- **`providers`**: user 的 providers 作为 base，project 的同名 key 整项替换 user 的
- **`mcp_servers`**: user 的 MCP servers 在前，project 的追加在后；同名 server（by name）中 project 的替换 user 的
- **`settings` 标量字段**: project `Some` 优先；project `None` 时使用 user 的值（包括 `restrict_workspace`）
- **`skills`**: project `Some` 时用 project 的；project `None` 时用 user 的

#### Scenario: settings restrict_workspace——project 优先
- **WHEN** user config 设置 `restrict_workspace = false`，project config 设置 `restrict_workspace = true`
- **THEN** 合并后 `restrict_workspace` SHALL 为 `Some(true)`

#### Scenario: settings restrict_workspace——user 补充
- **WHEN** user config 设置 `restrict_workspace = false`，project config 未设置（`None`）
- **THEN** 合并后 `restrict_workspace` SHALL 为 `Some(false)`

#### Scenario: settings restrict_workspace——双方均未设置
- **WHEN** user config 和 project config 均未设置 `restrict_workspace`
- **THEN** 合并后 `restrict_workspace` SHALL 为 `None`（由 resolve 填充默认值 `true`）

### Requirement: RawConfig resolve
`RawConfig` SHALL 提供 `pub fn resolve(self) -> Config` 方法，将所有 `Option` 字段填充为默认值，返回最终 `Config`。

#### Scenario: restrict_workspace 为 None
- **WHEN** 合并后 `restrict_workspace` 为 `None`
- **THEN** resolve SHALL 使用默认值 `true`

#### Scenario: restrict_workspace 为 Some(false)
- **WHEN** 合并后 `restrict_workspace` 为 `Some(false)`
- **THEN** resolve SHALL 使用 `false`
