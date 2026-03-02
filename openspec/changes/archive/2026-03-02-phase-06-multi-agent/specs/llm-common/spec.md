## REMOVED Requirements

### Requirement: use_name_field 相关逻辑
**Reason**: 连续 other-agent 消息在 `merge_consecutive_same_role` 后合并为单条 user 消息，`name` 字段无法归属给多个 agent。统一使用 `[agent_name]` 前缀标识 other-agent 身份。
**Migration**: 所有 provider 统一使用 `[agent_name]` content 前缀方式，不再支持 `name` 字段方式。
