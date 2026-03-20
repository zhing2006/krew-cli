## ADDED Requirements

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
