## Why

当前所有内建文件工具（read_file、write_file、edit_file、glob、grep）都通过 `validate_path` 强制执行 workspace 边界检查，限制只能访问会话工作目录内的文件。这导致 Agent 无法读取外部知识库、参考文档或项目目录外的任何文件——而这在实际使用中是非常常见且合理的需求（例如在当前工作区讨论游戏设计时，需要引用存放在另一个仓库中的策划文档）。

## What Changes

- 在 `settings.toml` 的 `[settings]` 中新增 `restrict_workspace` 布尔配置项（默认 `true`）
- 当 `restrict_workspace = false` 时，`validate_path` 跳过 workspace 边界检查，允许所有文件工具访问系统上的任意路径
- 当 `restrict_workspace = true`（默认）时，行为不变——现有安全边界完全保留
- 更新 config help 文本、示例配置文件及双语文档（README、MANUAL、PDD、TDD）

## Capabilities

### New Capabilities
- `restrict-workspace-setting`: 允许禁用 workspace 路径边界检查的配置选项

### Modified Capabilities
- `config-types`: 在 `Settings` 中添加 `restrict_workspace: bool` 字段
- `user-config`: 在 `RawSettings`、`UserSettings` 中添加 `restrict_workspace: Option<bool>` 字段，`merge_user` 添加合并逻辑，`resolve` 添加默认值填充
- `builtin-tools-readonly`: 将 `restrict_workspace` 标志透传到 `validate_path`（glob、grep、read_file）
- `edit-file-tool`: 将 `restrict_workspace` 标志透传到 `validate_path`
- `write-file-tool`: 将 `restrict_workspace` 标志透传到 `validate_path`（当前使用内联边界检查而非 `validate_path`）
- `config-help-command`: 在硬编码帮助文本的 settings 完整字段列表中添加 `restrict_workspace`
- `tool-registry`: 修改 `create_readonly_registry` 和 `create_full_registry` 工厂函数签名，增加 `restrict_workspace: bool` 参数

## Impact

- **代码**: `krew-config`（settings 结构体 + 默认值）、`krew-tools`（`validate_path` 签名、5 个内建工具文件）、`krew-core`（将配置传入 tool context）
- **配置**: `config.example.toml`、硬编码的 config help 文本
- **文档**: README（EN/CN）、MANUAL（EN/CN）、PDD、TDD
- **安全性**: 默认行为无变化；用户主动关闭即接受全文件系统访问风险
