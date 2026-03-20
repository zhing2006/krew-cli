## ADDED Requirements

### Requirement: SkillsConfig 结构体
`krew-config` SHALL 定义 `SkillsConfig` 结构体，包含字段：
- `enabled: bool`（默认 `true`）：是否启用 skill 功能
- `extra_paths: Vec<String>`：额外的 skill 扫描路径

#### Scenario: 默认配置
- **WHEN** settings.toml 中没有 `[skills]` 配置节
- **THEN** `SkillsConfig` SHALL 使用默认值：`enabled = true`、`extra_paths` 为空

#### Scenario: 自定义配置
- **WHEN** settings.toml 包含 `[skills]` 配置节 `enabled = true` 和 `extra_paths = ["/opt/skills"]`
- **THEN** SHALL 正确解析所有字段

#### Scenario: 禁用 skills
- **WHEN** settings.toml 包含 `[skills]` 配置节 `enabled = false`
- **THEN** 系统 SHALL 跳过 skill 发现和 catalog 注入

### Requirement: /skills 命令实现
`/skills` 斜杠命令 SHALL 在 viewport 上方显示当前可用的所有 skills 信息。显示内容 SHALL 包含每个 skill 的 name、description 和来源路径。

#### Scenario: 显示可用 skills
- **WHEN** 用户输入 `/skills` 且有 3 个可用 skills
- **THEN** 系统 SHALL 在 viewport 上方显示 3 个 skill 的名称、描述和来源路径

#### Scenario: 无可用 skills
- **WHEN** 用户输入 `/skills` 且没有可用 skills
- **THEN** 系统 SHALL 显示信息 "No skills available"

#### Scenario: skills 功能被禁用
- **WHEN** 用户输入 `/skills` 且 `skills.enabled = false`
- **THEN** 系统 SHALL 显示信息 "Skills feature is disabled"
