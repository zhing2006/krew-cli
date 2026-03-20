## ADDED Requirements

### Requirement: Session TOML serialization
The system SHALL serialize a session to TOML format with a `[session]` table containing metadata (id, cwd, agents, total_tokens_used, created_at, updated_at) and a `[[messages]]` array containing each message (role, content, agent_name, addressee, usage, created_at).

#### Scenario: Serialize a session with user and assistant messages
- **WHEN** a session with id "abc123", cwd "/project", agents ["gpt", "opus"], and 2 messages is serialized
- **THEN** the output TOML SHALL contain a `[session]` table with all metadata fields and 2 `[[messages]]` entries with correct role, content, and timestamps

#### Scenario: Serialize empty session
- **WHEN** a new session with no messages is serialized
- **THEN** the output TOML SHALL contain the `[session]` table with metadata and zero `[[messages]]` entries

### Requirement: Session TOML deserialization
The system SHALL deserialize a TOML file back into a session structure, restoring all metadata and messages.

#### Scenario: Deserialize a valid session file
- **WHEN** a valid `.toml` file conforming to TDD §3.6.1 format is read
- **THEN** the system SHALL reconstruct the session with correct id, cwd, agents, messages, and token usage

#### Scenario: Deserialize a corrupted file
- **WHEN** a `.toml` file contains invalid or unparseable content
- **THEN** the system SHALL return a `StorageError` with a descriptive message

### Requirement: Save session to file
The system SHALL write a session to `.krew/sessions/<id>.toml` using atomic write (write to `.tmp` then rename).

#### Scenario: Save session atomically
- **WHEN** `save_session()` is called with a session
- **THEN** the system SHALL write to `<id>.toml.tmp` first, then rename to `<id>.toml`

#### Scenario: Save session creates directory
- **WHEN** `save_session()` is called and `.krew/sessions/` does not exist
- **THEN** the system SHALL create the directory before writing

### Requirement: Load session from file
The system SHALL read a session from a `.toml` file and return the deserialized session data.

#### Scenario: Load existing session
- **WHEN** `load_session()` is called with a valid session file path
- **THEN** the system SHALL return the deserialized session

#### Scenario: Load non-existent session
- **WHEN** `load_session()` is called with a path that does not exist
- **THEN** the system SHALL return a `StorageError::Io` error

### Requirement: List sessions
The system SHALL scan `.krew/sessions/` and return session summaries sorted by `updated_at` descending.

#### Scenario: List sessions in a directory with multiple sessions
- **WHEN** `list_sessions()` is called on a directory containing 3 session files
- **THEN** the system SHALL return 3 session summaries sorted by most recently updated first

#### Scenario: List sessions in an empty directory
- **WHEN** `list_sessions()` is called on a directory with no `.toml` files
- **THEN** the system SHALL return an empty list

#### Scenario: List sessions with a corrupted file
- **WHEN** `list_sessions()` scans a directory containing a corrupted `.toml` file
- **THEN** the system SHALL skip the corrupted file and return summaries for valid sessions only

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
