## ADDED Requirements

### Requirement: Session creation on startup
The system SHALL create a new session with a generated UUID when the application starts (unless resuming a previous session).

#### Scenario: Normal startup
- **WHEN** the application starts without `--resume`
- **THEN** a new session SHALL be created with a UUID id, current working directory, configured agent names, empty messages, and current timestamp

#### Scenario: Session ID displayed in header
- **WHEN** a new session is created
- **THEN** the startup header SHALL display the session ID (first 8 characters)

### Requirement: Real-time session persistence
The system SHALL save the session to disk after each message is added to the conversation.

#### Scenario: Save after user message
- **WHEN** the user sends a message
- **THEN** the session file SHALL be updated with the new user message

#### Scenario: Save after agent response
- **WHEN** an agent completes its response (AgentEvent::Done)
- **THEN** the session file SHALL be updated with the assistant message and updated token usage

#### Scenario: Save failure does not crash
- **WHEN** saving the session to disk fails (e.g., disk full, permissions)
- **THEN** the system SHALL log a warning and continue operation without crashing

### Requirement: Session resume from file
The system SHALL restore a session from disk, repopulating the conversation message history.

#### Scenario: Resume restores messages
- **WHEN** a session is resumed (via `/resume` or `--resume`)
- **THEN** the message history SHALL be loaded from the session file and made available for subsequent agent calls

#### Scenario: Resume restores token usage
- **WHEN** a session is resumed
- **THEN** the cumulative token usage per agent SHALL be restored from the session file

#### Scenario: Resume updates session metadata
- **WHEN** a session is resumed
- **THEN** the `updated_at` timestamp SHALL be updated to the current time
