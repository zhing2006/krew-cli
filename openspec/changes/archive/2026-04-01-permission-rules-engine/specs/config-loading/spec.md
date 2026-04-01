## MODIFIED Requirements

### Requirement: Config 内置默认值
`Config::default()` 返回的默认配置 SHALL 更新为：
- `settings.allow_rules` = `[]`（空 Vec）
- `settings.deny_rules` = `[]`（空 Vec）
- `settings.ask_rules` = `[]`（空 Vec）
- 不再包含 `settings.shell_allow_commands` 和 `settings.fetch_allow_domains` 字段

#### Scenario: 默认配置无规则
- **WHEN** 调用 `Config::default()`
- **THEN** `settings.allow_rules`、`settings.deny_rules`、`settings.ask_rules` SHALL 均为空 Vec

### Requirement: 启动时加载流程
`krew-cli` 的 `load_config()` 函数 SHALL 在合并 user 和 project 配置时，合并 `allow_rules`、`deny_rules`、`ask_rules` 列表（两个来源的规则拼接，不去重）。

#### Scenario: 合并 user 和 project 规则
- **WHEN** user config 包含 `[[deny_rules]] tool = "shell" pattern = "rm *"` 且 project config 包含 `[[deny_rules]] tool = "shell" pattern = "dd *"`
- **THEN** 合并后的 `deny_rules` SHALL 包含两条规则
