## ADDED Requirements

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
