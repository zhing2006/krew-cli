## MODIFIED Requirements

### Requirement: Settings 结构体字段

`Settings` 结构体 SHALL 包含 `restrict_workspace: bool` 字段，默认值为 `true`。该字段 SHALL 可从 TOML 配置文件的 `[settings]` 段反序列化。

#### Scenario: 从 TOML 反序列化 restrict_workspace
- **WHEN** 配置文件包含 `restrict_workspace = false`
- **THEN** `Settings.restrict_workspace` SHALL 为 `false`

#### Scenario: 缺省时使用默认值
- **WHEN** 配置文件中未设置 `restrict_workspace`
- **THEN** `Settings.restrict_workspace` SHALL 为 `true`
