## MODIFIED Requirements

### Requirement: /new command
The `/new` command (also `/clear`) SHALL save the current session to disk, clear the conversation context, create a new session with a fresh UUID, clear the screen, and display the new header with the new session ID.

#### Scenario: Execute /new with active session
- **WHEN** the user runs `/new` during an active session with messages
- **THEN** the current session SHALL be saved, conversation messages and token usage SHALL be cleared, a new session id SHALL be generated, the screen SHALL be cleared, and the header SHALL show the new session id

#### Scenario: Execute /new with empty session
- **WHEN** the user runs `/new` on a session with no messages
- **THEN** the empty session SHALL NOT be saved to disk, a new session id SHALL be generated, and the screen SHALL be cleared with a new header

## ADDED Requirements

### Requirement: /resume command
The `/resume` command SHALL list recent sessions and allow the user to select one to resume.

#### Scenario: Execute /resume with available sessions
- **WHEN** the user runs `/resume` and there are saved sessions
- **THEN** the system SHALL display a numbered list of sessions (most recent first) showing: index, date/time, agent names, and first message preview (truncated to 40 chars)

#### Scenario: Execute /resume with no saved sessions
- **WHEN** the user runs `/resume` and there are no saved sessions
- **THEN** the system SHALL display an info message: "No saved sessions found"

#### Scenario: User selects a session to resume
- **WHEN** the user inputs a valid session number after `/resume` listing
- **THEN** the current session SHALL be saved (if non-empty), the selected session SHALL be loaded, and a confirmation message SHALL be displayed

#### Scenario: /resume shows help text
- **WHEN** the `/help` command lists available commands
- **THEN** `/resume` SHALL be described as "Resume a previous session"
