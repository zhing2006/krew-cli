## ADDED Requirements

### Requirement: restrict_workspace 配置项

系统 SHALL 在 `[settings]` 中支持 `restrict_workspace` 布尔配置项，控制是否对内建文件工具执行 workspace 路径边界检查。

#### Scenario: 默认启用 workspace 限制
- **WHEN** 配置文件中未设置 `restrict_workspace`
- **THEN** 系统 SHALL 以 `restrict_workspace = true` 运行，所有文件工具仅允许访问 cwd 内的路径

#### Scenario: 显式关闭 workspace 限制
- **WHEN** 用户在 `settings.toml` 中设置 `restrict_workspace = false`
- **THEN** 所有内建文件工具（read_file、write_file、edit_file、glob、grep）SHALL 允许访问系统上任意路径，不再执行 workspace 边界检查

#### Scenario: 显式开启 workspace 限制
- **WHEN** 用户在 `settings.toml` 中设置 `restrict_workspace = true`
- **THEN** 行为与默认值相同，所有文件工具仅允许访问 cwd 内的路径

### Requirement: validate_path 支持跳过边界检查

`validate_path` 函数 SHALL 接受 `restrict` 参数。当 `restrict = false` 时，SHALL 仅执行路径解析（canonicalize），不执行 `starts_with(cwd)` 边界检查。

#### Scenario: restrict = true 时拒绝 workspace 外路径
- **WHEN** 调用 `validate_path(path, cwd, true)` 且 path 解析后不在 cwd 内
- **THEN** SHALL 返回错误 "path is outside the workspace boundary"

#### Scenario: restrict = false 时允许 workspace 外路径
- **WHEN** 调用 `validate_path(path, cwd, false)` 且 path 解析后不在 cwd 内
- **THEN** SHALL 返回解析后的路径，不报错

#### Scenario: restrict = false 时仍然解析路径
- **WHEN** 调用 `validate_path(path, cwd, false)` 且 path 不存在
- **THEN** SHALL 返回路径解析错误（与 restrict = true 时行为一致）
