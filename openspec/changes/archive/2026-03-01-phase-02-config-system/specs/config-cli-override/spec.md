## ADDED Requirements

### Requirement: Config::apply_cli_overrides() 方法
`krew-config` SHALL 在 `Config` 上实现 `pub fn apply_cli_overrides(&mut self, agents: Option<&str>, approval_mode: Option<&str>) -> Result<(), ConfigError>` 方法，用 CLI 参数覆盖配置文件中的设定。

#### Scenario: 无覆盖参数时配置不变
- **WHEN** 调用 `apply_cli_overrides(None, None)`
- **THEN** 配置 SHALL 保持不变

### Requirement: --agents 过滤 agent 列表
当 `agents` 参数为 `Some("name1,name2")` 时，`apply_cli_overrides` SHALL：
1. 按逗号分割参数值，去除空格
2. 过滤 `config.agents`，仅保留 `name` 在列表中的 agent
3. 更新 `config.settings.reply_order`，仅保留列表中存在的名称（保持参数中的顺序）

#### Scenario: 过滤为两个 agent
- **WHEN** 配置中有 5 个 agent，调用 `apply_cli_overrides(Some("gpt,opus"), None)`
- **THEN** `config.agents` SHALL 仅包含 name 为 `"gpt"` 和 `"opus"` 的两个 agent
- **AND** `config.settings.reply_order` SHALL 为 `["gpt", "opus"]`

#### Scenario: 指定的 agent 不存在
- **WHEN** 调用 `apply_cli_overrides(Some("gpt,nonexistent"), None)` 且 `"nonexistent"` 不在配置中
- **THEN** SHALL 返回 `Err(ConfigError::Validation(...))` 且错误信息包含 `"nonexistent"`

### Requirement: --approval-mode 覆盖审批策略
当 `approval_mode` 参数为 `Some(mode_str)` 时，`apply_cli_overrides` SHALL 解析该字符串为 `ApprovalMode` 枚举值并覆盖 `config.settings.approval_mode`。

#### Scenario: 覆盖为 full-auto
- **WHEN** 调用 `apply_cli_overrides(None, Some("full-auto"))`
- **THEN** `config.settings.approval_mode` SHALL 为 `ApprovalMode::FullAuto`

#### Scenario: 无效的审批模式
- **WHEN** 调用 `apply_cli_overrides(None, Some("invalid"))`
- **THEN** SHALL 返回 `Err(ConfigError::Validation(...))` 且错误信息说明有效选项为 `suggest`、`auto-edit`、`full-auto`
