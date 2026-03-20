## ADDED Requirements

### Requirement: Command file discovery
The system SHALL scan the `.krew/commands/` directory (relative to session cwd) at startup and register all `.md` files as custom slash commands. Subdirectories SHALL be scanned recursively.

#### Scenario: Flat command file
- **WHEN** `.krew/commands/commit.md` exists
- **THEN** the system SHALL register it as custom command `/commit`

#### Scenario: Nested command file
- **WHEN** `.krew/commands/review/pr.md` exists
- **THEN** the system SHALL register it as custom command `/review:pr`

#### Scenario: Deeply nested command file
- **WHEN** `.krew/commands/git/remote/push.md` exists
- **THEN** the system SHALL register it as custom command `/git:remote:push`

#### Scenario: No commands directory
- **WHEN** `.krew/commands/` directory does not exist
- **THEN** the system SHALL proceed normally with an empty custom command registry

#### Scenario: Non-md files ignored
- **WHEN** `.krew/commands/` contains files without `.md` extension
- **THEN** the system SHALL ignore those files

### Requirement: Frontmatter parsing
The system SHALL parse YAML frontmatter from command files. Frontmatter is delimited by `---` lines at the start of the file. Supported fields: `description` (string) and `argument-hint` (string). Both fields are optional.

#### Scenario: Full frontmatter
- **WHEN** a command file starts with `---\ndescription: Create a commit\nargument-hint: [message]\n---`
- **THEN** the system SHALL extract `description` as "Create a commit" and `argument-hint` as "[message]"

#### Scenario: Partial frontmatter
- **WHEN** a command file has frontmatter with only `description` field
- **THEN** the system SHALL extract `description` and use empty string for `argument-hint`

#### Scenario: No frontmatter
- **WHEN** a command file has no `---` delimiters at the start
- **THEN** the system SHALL use the entire file content as command body, with empty description and argument-hint

### Requirement: Argument substitution
The system SHALL replace argument placeholders in the command body before sending. `$ARGUMENTS` SHALL be replaced with the full argument string. `$1`, `$2`, etc. SHALL be replaced with positional arguments (whitespace-split). Unreferenced positional placeholders SHALL be replaced with empty string.

#### Scenario: $ARGUMENTS substitution
- **WHEN** user runs `/commit fix typo in readme` and command body contains `$ARGUMENTS`
- **THEN** `$ARGUMENTS` SHALL be replaced with "fix typo in readme"

#### Scenario: Positional arguments
- **WHEN** user runs `/deploy staging v2.0` and command body contains `$1` and `$2`
- **THEN** `$1` SHALL be replaced with "staging" and `$2` SHALL be replaced with "v2.0"

#### Scenario: Missing positional argument
- **WHEN** user runs `/deploy staging` and command body contains `$1` and `$2`
- **THEN** `$1` SHALL be replaced with "staging" and `$2` SHALL be replaced with empty string

#### Scenario: No arguments provided
- **WHEN** user runs `/commit` with no arguments and command body contains `$ARGUMENTS`
- **THEN** `$ARGUMENTS` SHALL be replaced with empty string

### Requirement: Command execution flow
After argument substitution and bash preprocessing, the expanded command text SHALL be routed through `parse_input()` for `@agent` addressing and sent as a normal user message.

#### Scenario: Command with @agent addressing
- **WHEN** a custom command body expands to `@coder review this code`
- **THEN** the system SHALL route the message to agent "coder" via normal `parse_input()` routing

#### Scenario: Command without @agent addressing
- **WHEN** a custom command body expands to `summarize the changes` (no @ prefix)
- **THEN** the system SHALL route the message to `Addressee::LastRespondent` (same as normal input)

### Requirement: Built-in command priority
Built-in slash commands SHALL always take priority over custom commands with the same name. Custom commands SHALL NOT be able to override built-in commands.

#### Scenario: Name collision with built-in
- **WHEN** both built-in `/help` and custom `.krew/commands/help.md` exist
- **THEN** `/help` SHALL execute the built-in command; the custom command SHALL be ignored

#### Scenario: No collision
- **WHEN** custom `/review` exists and no built-in `/review` exists
- **THEN** `/review` SHALL execute the custom command

### Requirement: Custom command in /help
The `/help` command SHALL display custom commands after built-in commands, with a "Custom commands:" subheading if any custom commands exist.

#### Scenario: Help with custom commands
- **WHEN** user runs `/help` and custom commands `/commit` (description: "Create a commit") and `/review:pr` exist
- **THEN** the output SHALL show built-in commands first, then a "Custom commands:" subheading, followed by custom command entries showing name and description

#### Scenario: Help with no custom commands
- **WHEN** user runs `/help` and no custom commands exist
- **THEN** the output SHALL show only built-in commands with no "Custom commands:" section
