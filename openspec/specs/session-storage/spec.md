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
