## ADDED Requirements

### Requirement: Input history file format
The system SHALL store input history in `.krew/history` as a plain text file with one entry per line. Newlines within entries SHALL be escaped as `\\n` and backslashes SHALL be escaped as `\\\\`.

#### Scenario: Single-line entry
- **WHEN** the user inputs "hello world"
- **THEN** the file SHALL contain one line: `hello world`

#### Scenario: Multi-line entry
- **WHEN** the user inputs "line1\nline2"
- **THEN** the file SHALL contain one line: `line1\\nline2`

#### Scenario: Entry with backslash
- **WHEN** the user inputs "path\\to\\file"
- **THEN** the file SHALL contain one line: `path\\\\to\\\\file`

### Requirement: Append-write on input
The system SHALL append each user input to `.krew/history` immediately when the message is sent.

#### Scenario: Append after user sends message
- **WHEN** the user sends a message
- **THEN** the entry SHALL be appended to `.krew/history` (creating the file if it does not exist)

#### Scenario: Consecutive duplicate suppression
- **WHEN** the user sends the same message as the previous entry
- **THEN** the entry SHALL NOT be appended (matching existing in-memory dedup behavior)

### Requirement: Load and truncate on startup
The system SHALL load the last `input_history_limit` entries from `.krew/history` on startup and rewrite the file with only those entries.

#### Scenario: File with more entries than limit
- **WHEN** `.krew/history` contains 500 entries and `input_history_limit` is 200
- **THEN** the system SHALL load the last 200 entries into memory and rewrite the file with only those 200 entries

#### Scenario: File does not exist
- **WHEN** `.krew/history` does not exist on startup
- **THEN** the system SHALL start with an empty input history and not create the file until the first input

#### Scenario: File with fewer entries than limit
- **WHEN** `.krew/history` contains 50 entries and `input_history_limit` is 200
- **THEN** the system SHALL load all 50 entries and rewrite the file with those 50 entries

### Requirement: Input history is session-independent
The input history SHALL persist across sessions. `/new` and `/resume` SHALL NOT clear or reset the input history.

#### Scenario: History survives /new
- **WHEN** the user runs `/new` after entering 10 inputs
- **THEN** the input history SHALL still contain all 10 entries and up-arrow navigation SHALL work
