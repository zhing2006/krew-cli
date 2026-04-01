## ADDED Requirements

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

## MODIFIED Requirements

### Requirement: Settings 结构体字段
`Settings` 结构体 SHALL 移除以下字段：
- `shell_allow_commands: Vec<String>` — **REMOVED**
- `fetch_allow_domains: Vec<String>` — **REMOVED**

SHALL 新增以下字段：
- `allow_rules: Vec<PermissionRule>` — 自动放行规则，默认为空 Vec
- `deny_rules: Vec<PermissionRule>` — 自动拒绝规则，默认为空 Vec
- `ask_rules: Vec<PermissionRule>` — 强制确认规则，默认为空 Vec

#### Scenario: 新字段反序列化
- **WHEN** TOML 包含 `[[allow_rules]]`、`[[deny_rules]]`、`[[ask_rules]]` 表
- **THEN** SHALL 正确反序列化为 `Vec<PermissionRule>`

#### Scenario: 规则字段缺省时为空
- **WHEN** TOML 中不包含任何规则相关配置
- **THEN** `allow_rules`、`deny_rules`、`ask_rules` SHALL 均为空 Vec

#### Scenario: 旧字段被忽略
- **WHEN** TOML 包含 `shell_allow_commands` 或 `fetch_allow_domains`
- **THEN** SHALL 被 serde 忽略（不影响反序列化），但通过 deprecated field 检测机制触发启动警告

## ADDED Requirements

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
