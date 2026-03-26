## MODIFIED Requirements

### Requirement: krew config help 子命令
`krew-cli` SHALL 提供 `krew config help` 子命令，在标准输出打印完整的 krew 配置手册（英文）。手册内容 SHALL 为硬编码的纯文本，与代码中的配置模型完全一致。该命令 SHALL 不读取任何配置文件，不需要网络访问，直接打印静态文本后以退出码 0 退出。

#### Scenario: 执行 krew config help
- **WHEN** 执行 `krew config help`
- **THEN** SHALL 在标准输出打印完整的配置手册文本（英文），退出码为 0

#### Scenario: 手册包含 settings 完整字段说明
- **WHEN** 执行 `krew config help`
- **THEN** 输出 SHALL 包含 `[settings]` 表的所有字段：approval_mode、reply_order（仅 project）、auto_compact_threshold、compact_keep_rounds、input_history_limit、paste_burst_detection、worker_threads、other_agent_role、shell_allow_commands、fetch_allow_domains、agent_to_agent_routing、agent_to_agent_max_rounds、language、**restrict_workspace**，以及各字段的默认值（与代码常量一致）
