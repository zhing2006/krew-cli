## ADDED Requirements

### Requirement: Settings update_check 字段
`Settings` 结构体 SHALL 包含 `update_check: bool` 字段，默认值为 `true`。该字段 SHALL 可从 TOML 配置文件的 `[settings]` 段反序列化。`RawSettings` 和 `UserSettings` 中对应字段 SHALL 为 `Option<bool>`，参与分层配置合并。

#### Scenario: 从 TOML 反序列化 update_check
- **WHEN** 配置文件包含 `update_check = false`
- **THEN** `Settings.update_check` SHALL 为 `false`

#### Scenario: 缺省时使用默认值
- **WHEN** 配置文件中未设置 `update_check`
- **THEN** `Settings.update_check` SHALL 为 `true`

#### Scenario: 分层合并
- **WHEN** user config 设置 `update_check = false`，project config 未设置
- **THEN** 合并后 `Settings.update_check` SHALL 为 `false`
