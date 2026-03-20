## MODIFIED Requirements

### Requirement: Session TOML 序列化
系统 SHALL 将 session 序列化为 TOML 格式，包含 `[session]` 表（id、cwd、agents、total_tokens_used、created_at、updated_at 等元数据）和 `[[messages]]` 数组（每条消息的 role、content、agent_name、addressee、whisper_targets、usage、created_at）。`whisper_targets` 字段 SHALL 序列化为 TOML 原生数组（如 `["opus", "gemini"]`），为 `None` 时跳过。

#### Scenario: 序列化包含用户和 assistant 消息的 session
- **WHEN** 一个包含 id "abc123"、cwd "/project"、agents ["gpt", "opus"] 和 2 条消息的 session 被序列化
- **THEN** 输出 TOML SHALL 包含带所有元数据字段的 `[session]` 表和 2 个 `[[messages]]` 条目

#### Scenario: 序列化密语消息
- **WHEN** 一条消息的 `whisper_targets = Some(["opus", "gemini"])`
- **THEN** `[[messages]]` 条目 SHALL 包含 `whisper_targets = ["opus", "gemini"]`（TOML 原生数组格式）

#### Scenario: 序列化非密语消息
- **WHEN** 一条消息的 `whisper_targets = None`
- **THEN** `[[messages]]` 条目 SHALL NOT 包含 `whisper_targets` 字段

### Requirement: Session TOML 反序列化
系统 SHALL 将 TOML 文件反序列化回 session 结构，恢复所有元数据和消息，包括 `whisper_targets`。

#### Scenario: 反序列化包含密语消息的有效 session 文件
- **WHEN** `.toml` 文件包含带 `whisper_targets = ["opus", "gemini"]` 的 `[[messages]]` 条目
- **THEN** 系统 SHALL 将 `whisper_targets` 重建为 `Some(vec!["opus", "gemini"])`

#### Scenario: 反序列化不含密语字段的旧版 session 文件
- **WHEN** 一个密语功能之前版本的 `.toml` 文件不包含 `whisper_targets` 字段
- **THEN** 系统 SHALL 将所有消息视为 `whisper_targets = None`（向后兼容）
