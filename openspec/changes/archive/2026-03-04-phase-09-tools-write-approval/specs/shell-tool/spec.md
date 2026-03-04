## ADDED Requirements

### Requirement: shell tool handler
`krew-tools` SHALL implement a `ShellTool` struct implementing `ToolHandler`. The tool SHALL execute shell commands and return stdout/stderr. The tool name SHALL be `"shell"`.

#### Scenario: Simple command
- **WHEN** shell is called with `{ "command": "echo hello" }`
- **THEN** ToolResult SHALL contain `"hello\n"` and `is_error: false`

#### Scenario: Command fails
- **WHEN** shell is called with a command that exits with non-zero status
- **THEN** ToolResult SHALL contain stderr output and exit code, with `is_error: true`

### Requirement: shell cross-platform detection
ShellTool SHALL detect the appropriate shell executable for the current platform:

**Windows detection order:**
1. `KREW_BASH_PATH` environment variable
2. Search `PATH` for `bash.exe`, skipping `C:\Windows\System32\bash.exe` (WSL bash)
3. Hardcoded path: `C:\Program Files\Git\bin\bash.exe`
4. Hardcoded path: `C:\Program Files (x86)\Git\bin\bash.exe`
5. Error: "Git Bash not found. Install Git for Windows or set KREW_BASH_PATH."

**Unix detection:**
1. `KREW_BASH_PATH` environment variable
2. `$SHELL` environment variable
3. Fallback: `/bin/sh`

All commands SHALL be executed as `<shell> -c "<command>"`.

#### Scenario: Windows uses Git Bash
- **WHEN** running on Windows with Git for Windows installed
- **THEN** shell SHALL use Git Bash's `bash.exe`, NOT System32's WSL bash

#### Scenario: Windows KREW_BASH_PATH override
- **WHEN** `KREW_BASH_PATH` is set to a valid path
- **THEN** shell SHALL use that path regardless of other detection

#### Scenario: Unix uses SHELL
- **WHEN** running on Unix with `$SHELL=/bin/zsh`
- **THEN** shell SHALL use `/bin/zsh -c` to execute commands

### Requirement: shell timeout
Shell command execution SHALL have a configurable timeout, passed as an optional tool parameter `timeout_seconds` (default: 120).

#### Scenario: Command within timeout
- **WHEN** shell is called with a command completing in 2s and default timeout
- **THEN** ToolResult SHALL contain the command output

#### Scenario: Command exceeds timeout
- **WHEN** shell is called with a command exceeding the timeout
- **THEN** the process SHALL be killed and ToolResult SHALL contain a timeout error message with `is_error: true`

#### Scenario: Custom timeout
- **WHEN** shell is called with `{ "command": "cargo build", "timeout_seconds": 300 }`
- **THEN** the timeout SHALL be 300 seconds

### Requirement: shell output capture
Shell SHALL capture both stdout and stderr. Output SHALL be truncated if it exceeds 100KB, with a message indicating truncation.

#### Scenario: Combined output
- **WHEN** a command writes to both stdout and stderr
- **THEN** ToolResult SHALL contain both streams (stderr first, then stdout)

#### Scenario: Large output truncation
- **WHEN** command output exceeds 100KB
- **THEN** output SHALL be truncated with a `"[output truncated at 100KB]"` message

### Requirement: shell requires approval
`ShellTool::requires_approval()` SHALL return `true`.

#### Scenario: Approval flag
- **WHEN** checking `shell_tool.requires_approval()`
- **THEN** SHALL return `true`

### Requirement: shell Windows no console window
On Windows, shell SHALL spawn processes with `CREATE_NO_WINDOW` (0x08000000) creation flag to prevent visible console windows from flashing.

#### Scenario: No console flash
- **WHEN** shell executes a command on Windows
- **THEN** no visible console window SHALL appear

### Requirement: shell parameter schema
shell SHALL define its parameter schema with required field `command` (string) and optional field `timeout_seconds` (number, default 120).

#### Scenario: Schema definition
- **WHEN** `shell_tool.spec()` is called
- **THEN** parameters SHALL include `command` (required, string) and `timeout_seconds` (optional, number)

### Requirement: shell working directory
Shell SHALL execute commands in the session working directory (cwd).

#### Scenario: Working directory
- **WHEN** shell is called with `{ "command": "pwd" }` and cwd is `/home/user/project`
- **THEN** output SHALL contain `/home/user/project`
