## ADDED Requirements

### Requirement: Bash block execution
The system SHALL scan the command body (after argument substitution) for bash preprocessing blocks matching the pattern `` !`<command>` `` and execute each command via the system shell. The command's stdout SHALL replace the entire `` !`<command>` `` block in the text.

#### Scenario: Single bash block
- **WHEN** command body contains `` !`git status` ``
- **THEN** the system SHALL execute `git status`, capture stdout, and replace the block with the output

#### Scenario: Multiple bash blocks
- **WHEN** command body contains `` !`git status` `` and `` !`git diff --staged` ``
- **THEN** the system SHALL execute both commands sequentially and replace each block with its respective output

#### Scenario: No bash blocks
- **WHEN** command body contains no `` !`...` `` patterns
- **THEN** the system SHALL return the body unchanged

### Requirement: Bash block error handling
When a bash preprocessing command fails (non-zero exit code or spawn failure), the system SHALL replace the block with an error message and continue processing the rest of the command body. The command SHALL NOT be aborted.

#### Scenario: Command fails with non-zero exit
- **WHEN** a bash block `` !`git diff --staged` `` exits with code 1 and produces stderr "fatal: not a git repository"
- **THEN** the block SHALL be replaced with text like `[Error: command failed (exit 1): fatal: not a git repository]`

#### Scenario: Command not found
- **WHEN** a bash block `` !`nonexistent_cmd` `` fails to spawn
- **THEN** the block SHALL be replaced with text like `[Error: failed to execute: nonexistent_cmd]`

#### Scenario: Partial success
- **WHEN** command body contains two bash blocks, first succeeds and second fails
- **THEN** the first block SHALL be replaced with its output and the second SHALL be replaced with an error message; the full expanded body SHALL be sent as a message

### Requirement: Bash execution context
Bash preprocessing commands SHALL execute in the session's current working directory (same cwd as the krew-cli process).

#### Scenario: Cwd context
- **WHEN** krew-cli is running in `/home/user/project` and a bash block runs `` !`pwd` ``
- **THEN** the output SHALL be `/home/user/project`
