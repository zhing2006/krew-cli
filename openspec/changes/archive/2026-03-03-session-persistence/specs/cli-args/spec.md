## MODIFIED Requirements

### Requirement: --resume CLI argument
The `--resume` CLI argument SHALL resume a session on startup. When provided without an ID (`--resume`), it SHALL resume the most recent session. When provided with an ID (`--resume <id>`), it SHALL resume the specified session.

#### Scenario: Resume most recent session
- **WHEN** the user starts krew with `--resume` (no ID)
- **THEN** the system SHALL load the most recently updated session and display a confirmation with the session ID

#### Scenario: Resume specific session by ID
- **WHEN** the user starts krew with `--resume abc123`
- **THEN** the system SHALL load the session with id "abc123" (matching by prefix)

#### Scenario: Resume with non-existent session ID
- **WHEN** the user starts krew with `--resume nonexistent`
- **THEN** the system SHALL display an error message and start a new session instead

#### Scenario: Resume with no saved sessions
- **WHEN** the user starts krew with `--resume` and no sessions exist
- **THEN** the system SHALL display an info message and start a new session
