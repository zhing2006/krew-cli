## MODIFIED Requirements

### Requirement: 手册包含 settings 完整字段说明
- **WHEN** 执行 `krew config help`
- **THEN** 输出 SHALL 包含 `[settings]` 表的所有字段：approval_mode、reply_order（仅 project）、auto_compact_threshold、compact_keep_rounds、input_history_limit、paste_burst_detection、worker_threads、other_agent_role、shell_allow_commands、fetch_allow_domains、agent_to_agent_routing、agent_to_agent_max_rounds、language、restrict_workspace、**update_check**，以及各字段的默认值（与代码常量一致）

#### Scenario: 手册包含 update_check 字段说明
- **WHEN** 执行 `krew config help`
- **THEN** 输出 SHALL 包含 `update_check` 字段的说明，包括类型（bool）、默认值（true）、功能描述（启动时检查新版本）
