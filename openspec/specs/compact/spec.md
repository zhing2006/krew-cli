## ADDED Requirements

### Requirement: Manual compact command
The system SHALL support a `/compact [agent_name]` command that compresses conversation history into a summary using the specified agent's LLM. When no agent is specified, the system SHALL use the first agent in `reply_order`.

#### Scenario: Compact with explicit agent
- **WHEN** user enters `/compact opus`
- **THEN** the system compresses conversation history using the "opus" agent's LLM, keeps the last N rounds of conversation, and replaces older messages with a summary

#### Scenario: Compact without agent name
- **WHEN** user enters `/compact`
- **THEN** the system uses `reply_order[0]` as the compression agent

#### Scenario: Compact with invalid agent name
- **WHEN** user enters `/compact nonexistent`
- **THEN** the system displays an error message indicating the agent was not found

### Requirement: Conversation round preservation
The system SHALL preserve the last N conversation rounds during compaction, where N is configured by `compact_keep_rounds` (default: 10). A conversation round consists of one user message and all subsequent non-user messages until the next user message.

#### Scenario: Preserve last 10 rounds by default
- **WHEN** a session has 25 conversation rounds and user runs `/compact`
- **THEN** rounds 1-15 are compressed into a summary, rounds 16-25 are preserved intact

#### Scenario: Session has fewer rounds than keep threshold
- **WHEN** a session has 5 conversation rounds and `compact_keep_rounds` is 10
- **THEN** the system displays a message indicating there is nothing to compact

### Requirement: Summary injection
The system SHALL inject the compressed summary as a user-role message at the beginning of the message list, prefixed with `[Session History Summary]`.

#### Scenario: Summary placement after compact
- **WHEN** compaction completes successfully
- **THEN** the message list starts with a user message containing the summary, followed by the preserved conversation rounds

### Requirement: Pre-compact backup
The system SHALL create a backup of the complete session before compaction, saved as `{session_id}.pre-compact.{unix_timestamp}.toml` in the sessions directory.

#### Scenario: Backup creation
- **WHEN** user runs `/compact` successfully
- **THEN** a backup file is created in `.krew/sessions/` with the full pre-compact session data
- **THEN** the system displays the backup file path

### Requirement: Compact status display
The system SHALL display a status message after compaction showing the token reduction.

#### Scenario: Successful compact display
- **WHEN** compaction completes
- **THEN** the system displays a message like "Session compacted (45,000 tokens -> 3,200 tokens)" with the backup path

### Requirement: Auto-compact trigger
The system SHALL automatically trigger compaction when `prompt_tokens` from the last LLM response meets or exceeds `auto_compact_threshold`. The auto-compact SHALL execute before processing the next user message, using `reply_order[0]` as the compression agent.

#### Scenario: Auto-compact fires when threshold exceeded
- **WHEN** an agent reply returns `usage.prompt_tokens >= auto_compact_threshold`
- **AND** user sends the next message
- **THEN** the system auto-compacts before processing the user message and displays a status message

#### Scenario: Auto-compact disabled
- **WHEN** `auto_compact_threshold` is `None` or `0`
- **THEN** auto-compact never triggers regardless of token count

### Requirement: compact_keep_rounds configuration
The system SHALL support a `compact_keep_rounds` setting in `settings.toml` with a default value of 10.

#### Scenario: Custom keep rounds
- **WHEN** `settings.toml` contains `compact_keep_rounds = 5`
- **THEN** compaction preserves the last 5 conversation rounds

### Requirement: Enhanced /agents token display
The `/agents` command SHALL display per-agent token usage showing prompt_tokens and completion_tokens from each agent's last response.

#### Scenario: Agents command with token usage
- **WHEN** user enters `/agents` after multiple agent interactions
- **THEN** the display shows each agent with their token counts in the format "N tokens (X in / Y out)"
- **THEN** a total line shows the combined token usage

## MODIFIED Requirements

### Requirement: 对话轮次保留
系统 SHALL 在压缩时保留最近 N 个对话轮次，N 由 `compact_keep_rounds` 配置（默认 10）。一个对话轮次由一条用户消息和其后所有非用户消息（直到下一条用户消息）组成。带有 `whisper_targets` 的消息 SHALL 从压缩区提取并在 summary 之后保留，遵循与 skill activation 消息相同的提取-重插入模式。密语消息不会被送入压缩 LLM，也不包含在 summary 文本中。

#### Scenario: 默认保留最近 10 轮
- **WHEN** 会话有 25 个对话轮次，用户执行 `/compact`
- **THEN** 第 1-15 轮被压缩为 summary，第 16-25 轮原样保留

#### Scenario: 从压缩区提取密语消息
- **WHEN** 会话中密语消息与普通消息交错出现在压缩区
- **AND** 用户执行 `/compact`
- **THEN** 密语消息（带 `whisper_targets` 的用户消息及其对应的 agent 回复）SHALL 从压缩区提取，插入到 summary 和 skill 消息之后，完整保留其内容和 `whisper_targets` 元数据

#### Scenario: 保留区中的密语消息不变
- **WHEN** 密语消息存在于保留区（最近 N 轮）中
- **THEN** 它们 SHALL 在保留轮次中保持原位不变

#### Scenario: 压缩后消息顺序
- **WHEN** 压缩完成，压缩区中同时存在 skill 和密语消息
- **THEN** 最终消息列表 SHALL 为：`[Summary] + [skill messages] + [whisper messages] + [kept rounds]`

#### Scenario: 轮次少于保留阈值
- **WHEN** 会话有 5 个对话轮次且 `compact_keep_rounds` 为 10
- **THEN** 系统显示无内容可压缩的提示
